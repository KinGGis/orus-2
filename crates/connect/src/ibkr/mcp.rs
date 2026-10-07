//! Minimal MCP (Model Context Protocol) client over streamable HTTP.
//!
//! Only the subset IBKR requires is implemented: `initialize`, the
//! `notifications/initialized` acknowledgement, and `tools/call`. Responses may
//! come back as plain JSON or as a Server-Sent Events stream depending on the
//! tool, so both framings are handled.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::Mutex;

use wealthfolio_core::errors::{Error, Result};

use super::oauth::{IbkrTokenManager, IBKR_MCP_ENDPOINT};

const PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC client bound to one MCP session.
pub struct McpClient {
    http: reqwest::Client,
    tokens: Arc<IbkrTokenManager>,
    session_id: Mutex<Option<String>>,
    next_id: Mutex<u64>,
}

impl McpClient {
    pub fn new(http: reqwest::Client, tokens: Arc<IbkrTokenManager>) -> Self {
        Self {
            http,
            tokens,
            session_id: Mutex::new(None),
            next_id: Mutex::new(1),
        }
    }

    /// Invoke an MCP tool and return its decoded structured payload.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        self.ensure_session().await?;
        let params = json!({ "name": name, "arguments": arguments });
        let result = self.request("tools/call", params).await?;
        extract_tool_payload(name, &result)
    }

    /// Drop the cached session so the next call renegotiates one.
    pub async fn reset_session(&self) {
        *self.session_id.lock().await = None;
    }

    async fn ensure_session(&self) -> Result<()> {
        if self.session_id.lock().await.is_some() {
            return Ok(());
        }

        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "wealthfolio", "version": env!("CARGO_PKG_VERSION") },
        });
        self.request("initialize", params).await?;
        self.notify("notifications/initialized").await?;
        Ok(())
    }

    async fn notify(&self, method: &str) -> Result<()> {
        let body = json!({ "jsonrpc": "2.0", "method": method });
        self.send(&body).await?;
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value> {
        let id = {
            let mut next = self.next_id.lock().await;
            let current = *next;
            *next += 1;
            current
        };
        let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });

        let raw = self.send(&body).await?;
        let envelope = parse_jsonrpc_body(&raw).ok_or_else(|| {
            Error::Unexpected(format!(
                "IBKR MCP returned no JSON-RPC envelope for `{method}`"
            ))
        })?;

        if let Some(error) = envelope.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(Error::Unexpected(format!(
                "IBKR MCP rejected `{method}`: {message}"
            )));
        }

        Ok(envelope.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Post a JSON-RPC payload, retrying once if the session or token expired.
    async fn send(&self, body: &Value) -> Result<String> {
        match self.send_once(body).await {
            Ok(text) => Ok(text),
            Err(SendError::Unauthorized) => {
                // Access tokens live ten minutes and sessions are dropped with
                // them; renew both and replay the call once.
                self.tokens.refresh().await?;
                *self.session_id.lock().await = None;
                if body.get("method").and_then(Value::as_str) != Some("initialize") {
                    Box::pin(self.ensure_session()).await?;
                }
                match Box::pin(self.send_once(body)).await {
                    Ok(text) => Ok(text),
                    Err(SendError::Unauthorized) => Err(Error::Unexpected(
                        "IBKR MCP rejected the renewed access token".into(),
                    )),
                    Err(SendError::Other(e)) => Err(e),
                }
            }
            Err(SendError::Other(e)) => Err(e),
        }
    }

    async fn send_once(&self, body: &Value) -> std::result::Result<String, SendError> {
        let token = self.tokens.access_token().await.map_err(SendError::Other)?;

        let mut request = self
            .http
            .post(IBKR_MCP_ENDPOINT)
            .bearer_auth(token)
            .header("Content-Type", "application/json")
            // The server picks its framing from this header and answers with
            // SSE for streaming tools.
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION);

        if let Some(session) = self.session_id.lock().await.as_ref() {
            request = request.header("Mcp-Session-Id", session);
        }

        let response = request.json(body).send().await.map_err(|e| {
            SendError::Other(Error::Unexpected(format!("IBKR MCP request failed: {e}")))
        })?;

        let status = response.status();
        if let Some(session) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
        {
            *self.session_id.lock().await = Some(session);
        }

        let text = response.text().await.map_err(|e| {
            SendError::Other(Error::Unexpected(format!(
                "IBKR MCP returned an unreadable body (HTTP {status}): {e}"
            )))
        })?;

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(SendError::Unauthorized);
        }
        if !status.is_success() {
            return Err(SendError::Other(Error::Unexpected(format!(
                "IBKR MCP returned HTTP {status}: {}",
                truncate(&text, 400)
            ))));
        }
        Ok(text)
    }
}

enum SendError {
    Unauthorized,
    Other(Error),
}

/// Extract a JSON-RPC envelope from either a plain JSON body or an SSE stream.
fn parse_jsonrpc_body(raw: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        if value.get("jsonrpc").is_some() {
            return Some(value);
        }
    }

    // SSE framing: one or more `data:` lines, last envelope wins.
    raw.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|payload| serde_json::from_str::<Value>(payload.trim()).ok())
        .filter(|value| value.get("jsonrpc").is_some())
        .next_back()
}

/// Unwrap the tool result, preferring `structuredContent` and falling back to
/// the JSON embedded in the first text content block.
fn extract_tool_payload(tool: &str, result: &Value) -> Result<Value> {
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(Error::Unexpected(format!(
            "IBKR MCP tool `{tool}` reported an error: {}",
            truncate(&result.to_string(), 400)
        )));
    }

    if let Some(structured) = result.get("structuredContent") {
        if !structured.is_null() {
            return Ok(structured.clone());
        }
    }

    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find_map(|item| item.get("text").and_then(Value::as_str))
        })
        .ok_or_else(|| {
            Error::Unexpected(format!("IBKR MCP tool `{tool}` returned no usable content"))
        })?;

    serde_json::from_str(text).map_err(|e| {
        Error::Unexpected(format!(
            "IBKR MCP tool `{tool}` returned malformed JSON content: {e}"
        ))
    })
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        format!("{}…", &value[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_envelope() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        let parsed = parse_jsonrpc_body(raw).expect("envelope");
        assert_eq!(parsed["result"]["ok"], json!(true));
    }

    #[test]
    fn parses_sse_framed_envelope() {
        let raw = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":1}}\n\n";
        let parsed = parse_jsonrpc_body(raw).expect("envelope");
        assert_eq!(parsed["result"]["ok"], json!(1));
    }

    #[test]
    fn ignores_sse_keepalive_and_takes_last_envelope() {
        let raw = concat!(
            ": ping\n",
            "data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"final\":true}}\n\n"
        );
        let parsed = parse_jsonrpc_body(raw).expect("envelope");
        assert_eq!(parsed["result"]["final"], json!(true));
    }

    #[test]
    fn prefers_structured_content() {
        let result = json!({
            "structuredContent": { "positions": [] },
            "content": [{ "type": "text", "text": "{\"positions\":[{\"a\":1}]}" }]
        });
        let payload = extract_tool_payload("get_account_positions", &result).unwrap();
        assert_eq!(payload["positions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn falls_back_to_text_content() {
        let result = json!({
            "content": [{ "type": "text", "text": "{\"positions\":[{\"a\":1}]}" }]
        });
        let payload = extract_tool_payload("get_account_positions", &result).unwrap();
        assert_eq!(payload["positions"][0]["a"], json!(1));
    }

    #[test]
    fn surfaces_tool_errors() {
        let result = json!({ "isError": true, "content": [] });
        assert!(extract_tool_payload("get_account_summary", &result).is_err());
    }
}
