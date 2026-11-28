//! Model Context Protocol endpoint, so external AI agents (Claude Desktop,
//! Cursor, custom agents…) can read Orus data, query IBKR and run operations.
//!
//! Transport: stateless Streamable HTTP answering with plain JSON (no SSE).
//! Authentication: `Authorization: Bearer orus_…` (or `x-api-key: orus_…`) API keys created in the
//! settings, each with a READ or WRITE scope, or obtained through OAuth (see
//! `mcp_oauth`). Every tool call is journaled.

use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use chrono::{Duration, NaiveDate, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wealthfolio_core::accounts::AccountServiceTrait;
use wealthfolio_core::activities::{ActivityBulkMutationRequest, ActivityUpdate, NewActivity};
use wealthfolio_core::assets::NewAsset;
use wealthfolio_core::private_debt::PrivateDebtImportRequest;
use wealthfolio_core::quotes::Quote;
use wealthfolio_storage_postgres::mcp::{McpApiKey, McpKeyRepository, NewMcpApiKey, NewMcpAuditEntry};

use crate::{
    error::{ApiError, ApiResult},
    main_lib::AppState,
};

const KEY_PREFIX: &str = "orus_";
pub(super) const SCOPE_READ: &str = "READ";
pub(super) const SCOPE_WRITE: &str = "WRITE";
const IBKR_TOOL_PREFIX: &str = "ibkr_";
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_LOGGED_ARGUMENT_BYTES: usize = 8 * 1024;

const INSTRUCTIONS: &str = "Orus is the portfolio management system of the fund. \
Use get_accounts first to obtain account ids. Amounts are in the account currency unless stated. \
Tools prefixed with ibkr_ are relayed to Interactive Brokers. \
Write tools (activities, accounts, assets, manual quotes, syncs, imports, IBKR orders) need a WRITE key.";

/// Public MCP endpoint: authenticated by its own API keys, not by the session JWT.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/mcp",
            axum::routing::post(handle_mcp)
                .get(method_not_allowed)
                .delete(method_not_allowed),
        )
        .merge(super::mcp_oauth::public_router())
}

/// Key management and OAuth consent, mounted under the JWT-protected API.
pub fn management_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/mcp/keys", get(list_keys).post(create_key))
        .route("/mcp/keys/{id}", delete(revoke_key))
        .route("/mcp/calls", get(list_calls))
        .merge(super::mcp_oauth::consent_router())
}

async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

pub(super) fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    to_hex(&bytes)
}

pub(super) fn generate_secret() -> String {
    format!("{KEY_PREFIX}{}", random_hex(32))
}

pub(super) fn hash_secret(secret: &str) -> String {
    to_hex(&Sha256::digest(secret.as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn key_repository(state: &AppState) -> ApiResult<&Arc<McpKeyRepository>> {
    state.mcp_key_repository.as_ref().ok_or_else(|| {
        ApiError::NotImplemented("The MCP endpoint requires the PostgreSQL storage".to_string())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateKeyRequest {
    name: String,
    scope: String,
}

async fn create_key(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateKeyRequest>,
) -> ApiResult<Json<Value>> {
    let repo = key_repository(&state)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("The key needs a name".to_string()));
    }
    let scope = request.scope.to_uppercase();
    if scope != SCOPE_READ && scope != SCOPE_WRITE {
        return Err(ApiError::BadRequest("Scope must be READ or WRITE".to_string()));
    }
    let secret = generate_secret();
    let key = repo.create_key(NewMcpApiKey {
        name: name.to_string(),
        key_prefix: secret.chars().take(12).collect(),
        key_hash: hash_secret(&secret),
        scope,
    })?;
    Ok(Json(json!({ "key": key, "secret": secret })))
}

async fn list_keys(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<McpApiKey>>> {
    Ok(Json(key_repository(&state)?.list_keys()?))
}

async fn revoke_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    if key_repository(&state)?.revoke_key(&id)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

#[derive(Deserialize)]
struct CallsQuery {
    limit: Option<i64>,
}

async fn list_calls(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallsQuery>,
) -> ApiResult<Json<Value>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    Ok(Json(json!(key_repository(&state)?.list_calls(limit)?)))
}

// ---------------------------------------------------------------------------
// JSON-RPC transport
// ---------------------------------------------------------------------------

/// 401 pointing OAuth-capable clients to the discovery document (RFC 9728).
fn unauthorized(headers: &HeaderMap, message: &str) -> Response {
    let challenge = format!(
        "Bearer resource_metadata=\"{}\"",
        super::mcp_oauth::resource_metadata_url(headers)
    );
    let challenge = HeaderValue::from_str(&challenge).unwrap_or(HeaderValue::from_static("Bearer"));
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, challenge)],
        Json(json!({ "error": message })),
    )
        .into_response()
}

fn rpc_result(id: Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

fn rpc_error(status: StatusCode, id: Value, code: i64, message: &str) -> Response {
    (
        status,
        Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })),
    )
        .into_response()
}

async fn handle_mcp(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let Ok(repo) = key_repository(&state) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "The MCP endpoint requires the PostgreSQL storage")
            .into_response();
    };
    let Some(secret) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        // Some clients (e.g. Claude Desktop connectors) reserve `Authorization` for OAuth.
        .or_else(|| headers.get("x-api-key").and_then(|value| value.to_str().ok()))
        .map(str::trim)
        .filter(|secret| secret.starts_with(KEY_PREFIX))
    else {
        return unauthorized(&headers, "Missing Orus API key");
    };
    let key = match repo.authenticate(&hash_secret(secret)) {
        Ok(Some(key)) => key,
        Ok(None) => return unauthorized(&headers, "Invalid or revoked Orus API key"),
        Err(e) => {
            tracing::error!("MCP key authentication failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => return rpc_error(StatusCode::BAD_REQUEST, Value::Null, -32700, "Parse error"),
    };
    if message.is_array() {
        return rpc_error(StatusCode::BAD_REQUEST, Value::Null, -32600, "Batches are not supported");
    }
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response or malformed message: nothing to answer.
        return StatusCode::ACCEPTED.into_response();
    };
    let Some(id) = message.get("id").cloned() else {
        // Notifications (initialized, cancelled…) need no answer.
        return StatusCode::ACCEPTED.into_response();
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => rpc_result(id, initialize_result(&params)),
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(id, json!({ "tools": list_tools(&state, &key).await })),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return rpc_error(StatusCode::OK, id, -32602, "Missing tool name");
            };
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match call_tool(&state, repo, &key, name, arguments).await {
                Some(result) => rpc_result(id, result),
                None => rpc_error(StatusCode::OK, id, -32602, &format!("Unknown tool: {name}")),
            }
        }
        _ => rpc_error(StatusCode::OK, id, -32601, &format!("Method not found: {method}")),
    }
}

fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|v| SUPPORTED_PROTOCOL_VERSIONS.iter().find(|s| **s == v).copied())
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0])
}

fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    json!({
        "protocolVersion": negotiate_protocol_version(requested),
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "orus", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

fn native_read_tools() -> Vec<Value> {
    vec![tool(
        "get_private_debt_schedule",
        "Private debt cash-flow schedule (interest and principal, EXPECTED or settled). \
         With accountId and assetId returns the whole schedule of one loan; otherwise the flows \
         of the whole book falling due between from and to (default: 90 days ago to 1 year ahead).",
        json!({
            "type": "object",
            "properties": {
                "accountId": { "type": "string" },
                "assetId": { "type": "string" },
                "from": { "type": "string", "format": "date" },
                "to": { "type": "string", "format": "date" }
            }
        }),
    )]
}

fn native_write_tools() -> Vec<Value> {
    let symbol = json!({
        "type": "object",
        "description": "Asset of the activity. Omit for pure cash activities (DEPOSIT, WITHDRAWAL, FEE…).",
        "properties": {
            "id": { "type": "string", "description": "Existing asset id" },
            "symbol": { "type": "string" },
            "exchangeMic": { "type": "string" },
            "kind": { "type": "string" },
            "name": { "type": "string" },
            "quoteMode": { "type": "string", "enum": ["MARKET", "MANUAL"] },
            "quoteCcy": { "type": "string" },
            "instrumentType": { "type": "string" }
        }
    });
    let activity_properties = json!({
        "accountId": { "type": "string" },
        "symbol": symbol,
        "activityType": {
            "type": "string",
            "enum": ["BUY", "SELL", "DIVIDEND", "INTEREST", "DEPOSIT", "WITHDRAWAL", "TRANSFER_IN",
                     "TRANSFER_OUT", "FEE", "TAX", "SPLIT", "CREDIT", "ADJUSTMENT"]
        },
        "subtype": { "type": "string" },
        "activityDate": { "type": "string", "description": "ISO 8601 date or datetime" },
        "quantity": { "type": "number" },
        "unitPrice": { "type": "number" },
        "currency": { "type": "string" },
        "fee": { "type": "number" },
        "amount": { "type": "number" },
        "fxRate": { "type": "number" },
        "notes": { "type": "string" }
    });
    let mut update_properties = activity_properties.clone();
    update_properties["id"] = json!({ "type": "string" });

    vec![
        tool(
            "create_activity",
            "Record an activity (trade, cash movement, income, fee…) in an account. \
             Holdings and valuations are recalculated automatically.",
            json!({
                "type": "object",
                "properties": activity_properties,
                "required": ["accountId", "activityType", "activityDate", "currency"]
            }),
        ),
        tool(
            "update_activity",
            "Replace an existing activity. Send the complete activity, not only the changed fields.",
            json!({
                "type": "object",
                "properties": update_properties,
                "required": ["id", "accountId", "activityType", "activityDate", "currency"]
            }),
        ),
        tool(
            "delete_activity",
            "Delete an activity by id.",
            json!({
                "type": "object",
                "properties": { "activityId": { "type": "string" } },
                "required": ["activityId"]
            }),
        ),
        tool(
            "create_account",
            "Create a portfolio account. currency must be a 3-letter ISO fiat code (USD, EUR…); \
             crypto and stablecoins (USDT…) are assets held in the account, not its currency.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "accountType": { "type": "string", "enum": ["SECURITIES", "CASH", "CRYPTOCURRENCY"] },
                    "currency": { "type": "string" },
                    "group": { "type": "string" },
                    "trackingMode": { "type": "string", "enum": ["TRANSACTIONS", "HOLDINGS", "NOT_SET"] },
                    "platformId": { "type": "string" },
                    "accountNumber": { "type": "string" }
                },
                "required": ["name", "accountType", "currency"]
            }),
        ),
        tool(
            "update_account",
            "Change an account: rename, regroup, archive (isArchived) or deactivate it. \
             Only the fields sent are changed.",
            json!({
                "type": "object",
                "properties": {
                    "accountId": { "type": "string" },
                    "name": { "type": "string" },
                    "accountType": { "type": "string", "enum": ["SECURITIES", "CASH", "CRYPTOCURRENCY"] },
                    "group": { "type": "string" },
                    "isActive": { "type": "boolean" },
                    "isArchived": { "type": "boolean" },
                    "trackingMode": { "type": "string", "enum": ["TRANSACTIONS", "HOLDINGS", "NOT_SET"] },
                    "platformId": { "type": "string" },
                    "accountNumber": { "type": "string" }
                },
                "required": ["accountId"]
            }),
        ),
        tool(
            "save_activities",
            "Create, update and delete many activities in one call. Updates must be complete activities. \
             Entries that fail are listed in errors; the others are saved.",
            json!({
                "type": "object",
                "properties": {
                    "creates": {
                        "type": "array",
                        "items": { "type": "object", "properties": activity_properties }
                    },
                    "updates": {
                        "type": "array",
                        "items": { "type": "object", "properties": update_properties }
                    },
                    "deleteIds": { "type": "array", "items": { "type": "string" } }
                }
            }),
        ),
        tool(
            "create_asset",
            "Create an asset without recording an activity, e.g. an unlisted security priced manually. \
             Listed securities use kind INVESTMENT with instrumentType, instrumentSymbol and quoteMode MARKET.",
            json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["INVESTMENT", "PROPERTY", "VEHICLE", "COLLECTIBLE", "PRECIOUS_METAL",
                                 "PRIVATE_EQUITY", "LIABILITY", "OTHER"]
                    },
                    "name": { "type": "string" },
                    "displayCode": { "type": "string", "description": "Short code shown in the UI" },
                    "quoteMode": { "type": "string", "enum": ["MARKET", "MANUAL"] },
                    "quoteCcy": { "type": "string" },
                    "instrumentType": {
                        "type": "string",
                        "enum": ["EQUITY", "CRYPTO", "FX", "OPTION", "METAL", "BOND"]
                    },
                    "instrumentSymbol": { "type": "string" },
                    "instrumentExchangeMic": { "type": "string" },
                    "notes": { "type": "string" }
                },
                "required": ["kind", "quoteMode", "quoteCcy"]
            }),
        ),
        tool(
            "set_manual_quote",
            "Record or replace the manual price of an asset on a date. Valuations are then rebuilt \
             in the background. open/high/low default to close.",
            json!({
                "type": "object",
                "properties": {
                    "assetId": { "type": "string" },
                    "date": { "type": "string", "format": "date" },
                    "close": { "type": "number" },
                    "currency": { "type": "string" },
                    "open": { "type": "number" },
                    "high": { "type": "number" },
                    "low": { "type": "number" },
                    "notes": { "type": "string" }
                },
                "required": ["assetId", "date", "close", "currency"]
            }),
        ),
        tool(
            "sync_ibkr",
            "Start a full Interactive Brokers synchronisation (activities, positions, NAV). \
             Runs in the background and returns immediately.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "recalculate_portfolio",
            "Rebuild the full portfolio history (snapshots, valuations, performance). Runs in the background.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "import_private_debt",
            "Import a private debt platform statement: loans and transactions. Reconciles estimated \
             flows with the real ones and regenerates the schedules. Rates are in percent.",
            json!({
                "type": "object",
                "properties": {
                    "accountId": { "type": "string" },
                    "platform": { "type": "string" },
                    "loans": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "projectId": { "type": "string" },
                                "borrower": { "type": "string" },
                                "purchaseDate": { "type": "string", "format": "date" },
                                "nextPaymentDate": { "type": "string", "format": "date" },
                                "rate": { "type": "number" },
                                "loyaltyBonus": { "type": "number" },
                                "projectType": { "type": "string" },
                                "totalMonths": { "type": "integer" },
                                "remainingMonths": { "type": "integer" },
                                "countryCode": { "type": "string" },
                                "status": { "type": "string" },
                                "nominal": { "type": "number" },
                                "currency": { "type": "string" }
                            },
                            "required": ["projectId", "borrower", "purchaseDate", "rate", "totalMonths",
                                         "remainingMonths", "nominal", "currency"]
                        }
                    },
                    "transactions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "timestamp": { "type": "string", "description": "YYYY-MM-DDTHH:MM:SS" },
                                "kind": { "type": "string" },
                                "projectId": { "type": "string" },
                                "amount": { "type": "number", "description": "Signed amount" },
                                "currency": { "type": "string" }
                            },
                            "required": ["timestamp", "kind", "amount", "currency"]
                        }
                    }
                },
                "required": ["accountId", "platform", "loans", "transactions"]
            }),
        ),
    ]
}

/// IBKR tools that only read data; everything else (orders, alerts,
/// watchlists, feedback…) changes state and needs a WRITE key.
fn is_read_only_ibkr_tool(name: &str) -> bool {
    name.starts_with("get_") || name.starts_with("search_") || name == "whats_new"
}

fn is_write_tool(name: &str) -> bool {
    match name.strip_prefix(IBKR_TOOL_PREFIX) {
        Some(ibkr_name) => !is_read_only_ibkr_tool(ibkr_name),
        None => native_write_tools()
            .iter()
            .any(|tool| tool["name"].as_str() == Some(name)),
    }
}

async fn list_tools(state: &Arc<AppState>, key: &McpApiKey) -> Vec<Value> {
    let can_write = key.scope == SCOPE_WRITE;
    let mut tools = Vec::new();

    for ai_tool in wealthfolio_ai::tools::read_only_tools(state.ai_environment.clone()) {
        let definition = ai_tool.definition(String::new()).await;
        tools.push(tool(&definition.name, &definition.description, definition.parameters));
    }
    tools.extend(native_read_tools());
    if can_write {
        tools.extend(native_write_tools());
    }

    match ibkr_tools(state).await {
        Ok(ibkr) => tools.extend(ibkr.into_iter().filter_map(|mut t| {
            let name = t.get("name")?.as_str()?.to_string();
            if !can_write && !is_read_only_ibkr_tool(&name) {
                return None;
            }
            t["name"] = json!(format!("{IBKR_TOOL_PREFIX}{name}"));
            let description = t.get("description").and_then(Value::as_str).unwrap_or_default();
            t["description"] = json!(format!("[IBKR] {description}"));
            Some(t)
        })),
        Err(e) => tracing::warn!("MCP: IBKR tools unavailable: {e}"),
    }
    tools
}

async fn ibkr_tools(state: &AppState) -> ApiResult<Vec<Value>> {
    let client = super::ibkr::create_ibkr_client(state)?;
    client
        .list_tools()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn text_result(text: String, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn json_text(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn loggable_arguments(arguments: &Value) -> Value {
    let size = arguments.to_string().len();
    if size > MAX_LOGGED_ARGUMENT_BYTES {
        json!({ "truncated": true, "bytes": size })
    } else {
        arguments.clone()
    }
}

/// Runs a tool and journals the call. `None` when the tool does not exist.
async fn call_tool(
    state: &Arc<AppState>,
    repo: &McpKeyRepository,
    key: &McpApiKey,
    name: &str,
    arguments: Value,
) -> Option<Value> {
    let started = Instant::now();
    let outcome = if is_write_tool(name) && key.scope != SCOPE_WRITE {
        Some(Err(format!("The API key '{}' is read-only: {name} needs a WRITE key", key.name)))
    } else {
        run_tool(state, name, arguments.clone()).await
    };
    let outcome = outcome?;

    let entry = NewMcpAuditEntry {
        key_id: Some(key.id),
        tool_name: name.to_string(),
        arguments: Some(loggable_arguments(&arguments)),
        success: outcome.is_ok(),
        error: outcome.as_ref().err().cloned(),
        duration_ms: i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX),
    };
    if let Err(e) = repo.record_call(entry) {
        tracing::warn!("MCP: failed to journal the call to {name}: {e}");
    }

    Some(match outcome {
        Ok(result) => result,
        Err(message) => text_result(message, true),
    })
}

fn parse_args<T: serde::de::DeserializeOwned>(arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|e| format!("Invalid arguments: {e}"))
}

fn ok_json<T: serde::Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    Ok(text_result(json_text(&value), false))
}

async fn run_tool(state: &Arc<AppState>, name: &str, arguments: Value) -> Option<Result<Value, String>> {
    if let Some(ibkr_name) = name.strip_prefix(IBKR_TOOL_PREFIX) {
        return Some(call_ibkr_tool(state, ibkr_name, arguments).await);
    }

    let result = match name {
        "get_private_debt_schedule" => private_debt_schedule(state, arguments),
        "create_activity" => match parse_args::<NewActivity>(arguments) {
            Ok(activity) => state
                .activity_service
                .create_activity(activity)
                .await
                .map_err(|e| e.to_string())
                .and_then(ok_json),
            Err(e) => Err(e),
        },
        "update_activity" => match parse_args::<ActivityUpdate>(arguments) {
            Ok(activity) => state
                .activity_service
                .update_activity(activity)
                .await
                .map_err(|e| e.to_string())
                .and_then(ok_json),
            Err(e) => Err(e),
        },
        "delete_activity" => match arguments.get("activityId").and_then(Value::as_str) {
            Some(id) => state
                .activity_service
                .delete_activity(id.to_string())
                .await
                .map_err(|e| e.to_string())
                .and_then(ok_json),
            None => Err("Missing activityId".to_string()),
        },
        "create_account" => create_account(state, arguments).await,
        "update_account" => update_account(state, arguments).await,
        "save_activities" => match parse_args::<ActivityBulkMutationRequest>(arguments) {
            Ok(request) => state
                .activity_service
                .bulk_mutate_activities(request)
                .await
                .map_err(|e| e.to_string())
                .and_then(ok_json),
            Err(e) => Err(e),
        },
        "create_asset" => match parse_args::<NewAsset>(arguments) {
            Ok(asset) => state
                .asset_service
                .create_asset(asset)
                .await
                .map_err(|e| e.to_string())
                .and_then(ok_json),
            Err(e) => Err(e),
        },
        "set_manual_quote" => set_manual_quote(state, arguments).await,
        "sync_ibkr" => super::ibkr::sync_ibkr(State(state.clone()))
            .await
            .map_err(|e| e.to_string())
            .and_then(|Json(response)| ok_json(response)),
        "recalculate_portfolio" => super::portfolio::recalculate_portfolio(State(state.clone()), None)
            .await
            .map_err(|e| e.to_string())
            .and_then(|_| ok_json(json!({ "message": "Portfolio history rebuild started" }))),
        "import_private_debt" => match parse_args::<PrivateDebtImportRequest>(arguments) {
            Ok(request) => super::private_debt::import_statement(State(state.clone()), Json(request))
                .await
                .map_err(|e| e.to_string())
                .and_then(|Json(summary)| ok_json(summary)),
            Err(e) => Err(e),
        },
        _ => return run_ai_tool(state, name, arguments).await,
    };
    Some(result)
}

async fn run_ai_tool(state: &Arc<AppState>, name: &str, arguments: Value) -> Option<Result<Value, String>> {
    let ai_tool = wealthfolio_ai::tools::read_only_tools(state.ai_environment.clone())
        .into_iter()
        .find(|t| t.name() == name)?;
    Some(
        ai_tool
            .call(arguments.to_string())
            .await
            .map(|text| text_result(text, false))
            .map_err(|e| e.to_string()),
    )
}

/// Account currencies are converted with FX rates and formatted as ISO 4217 codes, so a
/// stablecoin or crypto ticker (USDT, USDC…) would break both.
fn check_account_currency(currency: Option<&str>) -> Result<(), String> {
    match currency {
        Some(c) if c.len() == 3 && c.chars().all(|ch| ch.is_ascii_uppercase()) => Ok(()),
        other => Err(format!(
            "Invalid account currency {other:?}: use a 3-letter ISO 4217 fiat code (USD, EUR…). \
             Crypto and stablecoins such as USDT are assets held in the account: record them with \
             create_activity (symbol e.g. USDT-USD, instrumentType CRYPTO)."
        )),
    }
}

async fn create_account(state: &Arc<AppState>, mut arguments: Value) -> Result<Value, String> {
    check_account_currency(arguments.get("currency").and_then(Value::as_str))?;
    if let Some(object) = arguments.as_object_mut() {
        object.entry("isDefault").or_insert(json!(false));
        object.entry("isActive").or_insert(json!(true));
    }
    let account = parse_args::<crate::models::NewAccount>(arguments)?;
    let created = state
        .account_service
        .create_account(account.into())
        .await
        .map_err(|e| e.to_string())?;
    ok_json(crate::models::Account::from(created))
}

const UPDATABLE_ACCOUNT_FIELDS: [&str; 8] = [
    "name", "accountType", "group", "isActive", "isArchived", "trackingMode", "platformId", "accountNumber",
];

/// Applies the fields sent on top of the stored account, as the update replaces it whole.
async fn update_account(state: &Arc<AppState>, arguments: Value) -> Result<Value, String> {
    let id = arguments
        .get("accountId")
        .and_then(Value::as_str)
        .ok_or("Missing accountId")?
        .to_string();
    let existing = state.account_service.get_account(&id).map_err(|e| e.to_string())?;
    let mut merged =
        serde_json::to_value(crate::models::Account::from(existing)).map_err(|e| e.to_string())?;
    if let Some(changes) = arguments.as_object() {
        for field in UPDATABLE_ACCOUNT_FIELDS {
            if let Some(value) = changes.get(field) {
                merged[field] = value.clone();
            }
        }
    }
    let update = parse_args::<crate::models::AccountUpdate>(merged)?;
    let updated = state
        .account_service
        .update_account(update.into())
        .await
        .map_err(|e| e.to_string())?;
    ok_json(crate::models::Account::from(updated))
}

/// Goes through the HTTP handler so the valuation rebuild it triggers also runs.
async fn set_manual_quote(state: &Arc<AppState>, arguments: Value) -> Result<Value, String> {
    let asset_id = arguments
        .get("assetId")
        .and_then(Value::as_str)
        .ok_or("Missing assetId")?
        .to_string();
    let date = parse_date(&arguments, "date")?.ok_or("Missing date")?;
    let close = arguments.get("close").cloned().ok_or("Missing close")?;
    let price = |field: &str| arguments.get(field).cloned().unwrap_or_else(|| close.clone());
    let quote = parse_args::<Quote>(json!({
        "id": "",
        "assetId": asset_id,
        "timestamp": format!("{date}T12:00:00Z"),
        "open": price("open"),
        "high": price("high"),
        "low": price("low"),
        "close": close,
        "adjclose": close,
        "volume": 0,
        "currency": arguments.get("currency"),
        "dataSource": "MANUAL",
        "createdAt": Utc::now(),
        "notes": arguments.get("notes"),
    }))?;
    super::market_data::update_quote(Path(asset_id.clone()), State(state.clone()), Json(quote))
        .await
        .map_err(|e| e.to_string())?;
    ok_json(json!({
        "assetId": asset_id,
        "date": date,
        "close": close,
        "message": "Manual quote saved, portfolio valuation rebuild started"
    }))
}

fn parse_date(arguments: &Value, field: &str) -> Result<Option<NaiveDate>, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("{field} must be YYYY-MM-DD"))
        })
        .transpose()
}

fn private_debt_schedule(state: &AppState, arguments: Value) -> Result<Value, String> {
    let repo = state
        .debt_schedule_repository
        .as_ref()
        .ok_or("Private debt is not available on this storage")?;
    let account_id = arguments.get("accountId").and_then(Value::as_str);
    let asset_id = arguments.get("assetId").and_then(Value::as_str);
    let flows = match (account_id, asset_id) {
        (Some(account_id), Some(asset_id)) => repo.list_flows(account_id, asset_id),
        _ => {
            let today = Utc::now().date_naive();
            let from = parse_date(&arguments, "from")?.unwrap_or(today - Duration::days(90));
            let to = parse_date(&arguments, "to")?.unwrap_or(today + Duration::days(365));
            repo.list_flows_due(from, to)
        }
    }
    .map_err(|e| e.to_string())?;
    ok_json(flows)
}

async fn call_ibkr_tool(state: &AppState, name: &str, arguments: Value) -> Result<Value, String> {
    let client = super::ibkr::create_ibkr_client(state).map_err(|e| e.to_string())?;
    client.call_tool_raw(name, arguments).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_prefixed_random_and_hashed() {
        let a = generate_secret();
        let b = generate_secret();
        assert!(a.starts_with(KEY_PREFIX));
        assert_eq!(a.len(), KEY_PREFIX.len() + 64);
        assert_ne!(a, b);
        assert_eq!(hash_secret(&a), hash_secret(&a));
        assert_ne!(hash_secret(&a), hash_secret(&b));
        assert_eq!(hash_secret(&a).len(), 64);
    }

    #[test]
    fn only_ibkr_lookups_are_read_only() {
        assert!(!is_write_tool("ibkr_get_account_summary"));
        assert!(!is_write_tool("ibkr_search_contracts"));
        assert!(!is_write_tool("ibkr_whats_new"));
        assert!(is_write_tool("ibkr_create_order_instruction"));
        assert!(is_write_tool("ibkr_delete_alert"));
        assert!(is_write_tool("create_activity"));
        assert!(is_write_tool("import_private_debt"));
        assert!(check_account_currency(Some("USD")).is_ok());
        assert!(check_account_currency(Some("USDT")).is_err());
        assert!(check_account_currency(Some("usd")).is_err());
        assert!(check_account_currency(None).is_err());
        for name in ["update_account", "save_activities", "create_asset", "set_manual_quote"] {
            assert!(is_write_tool(name), "{name} must need a WRITE key");
        }
        assert!(!is_write_tool("get_holdings"));
        assert!(!is_write_tool("get_private_debt_schedule"));
    }

    #[test]
    fn protocol_version_falls_back_to_latest() {
        assert_eq!(negotiate_protocol_version(Some("2025-03-26")), "2025-03-26");
        assert_eq!(negotiate_protocol_version(Some("1999-01-01")), "2025-06-18");
        assert_eq!(negotiate_protocol_version(None), "2025-06-18");
    }

    #[test]
    fn large_arguments_are_not_journaled() {
        let small = json!({ "a": 1 });
        assert_eq!(loggable_arguments(&small), small);
        let large = json!({ "blob": "x".repeat(MAX_LOGGED_ARGUMENT_BYTES) });
        assert_eq!(loggable_arguments(&large)["truncated"], json!(true));
    }
}
