//! OAuth 2.0 client for the IBKR MCP server.
//!
//! IBKR exposes an open Dynamic Client Registration endpoint (RFC 7591) and a
//! public-client authorization-code flow with PKCE. `client_credentials` is
//! rejected for public clients, so a one-time human consent is required; the
//! returned refresh token then keeps the integration unattended.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use wealthfolio_core::errors::{Error, Result};

pub const IBKR_AUTHORIZE_URL: &str = "https://api.ibkr.com/oauth2/authorize";
pub const IBKR_TOKEN_URL: &str = "https://api.ibkr.com/oauth2/api/v1/token";
pub const IBKR_REGISTER_URL: &str = "https://api.ibkr.com/oauth2/register";

/// RFC 8707 resource indicator identifying the MCP deployment.
pub const IBKR_MCP_RESOURCE: &str = "https://api.ibkr.com/v1/api/mcp";

/// Endpoint that actually serves JSON-RPC for self-registered clients.
///
/// The documented `/v1/api/mcp` path answers `400 Unsupported client` unless
/// the caller is an IBKR-verified partner; `/v1/api/mcp-public` accepts
/// dynamically registered clients with the same token.
pub const IBKR_MCP_ENDPOINT: &str = "https://api.ibkr.com/v1/api/mcp-public";

/// Read-only scope. `mcp.write` and `mcp.orders.submit` are deliberately not
/// requested: this integration must never be able to place an order.
pub const IBKR_SCOPE: &str = "mcp.read";

/// Renew slightly before expiry so an in-flight sync never trips a 401.
const EXPIRY_SKEW_SECONDS: i64 = 60;

/// IBKR's edge rejects requests without a recognisable `User-Agent` with an
/// HTML `403`, so every call must carry one. `reqwest` sends none by default.
const USER_AGENT: &str = concat!("Wealthfolio/", env!("CARGO_PKG_VERSION"));

/// Build an HTTP client configured for IBKR's edge.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_default()
}

/// Persistence for the long-lived refresh token.
///
/// IBKR rotates the refresh token on every renewal, so [`save_refresh_token`]
/// is called on each refresh and must durably overwrite the previous value.
///
/// [`save_refresh_token`]: IbkrTokenStore::save_refresh_token
pub trait IbkrTokenStore: Send + Sync {
    fn load_refresh_token(&self) -> Result<Option<String>>;
    fn save_refresh_token(&self, token: &str) -> Result<()>;
    fn clear_refresh_token(&self) -> Result<()>;
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistrationResponse {
    client_id: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// A PKCE challenge pair. The verifier must be retained between the
/// authorization redirect and the token exchange.
#[derive(Debug, Clone)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

/// Generate a fresh S256 PKCE pair.
pub fn generate_pkce() -> PkcePair {
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = B64URL.encode(bytes);
    let challenge = B64URL.encode(Sha256::digest(verifier.as_bytes()));
    PkcePair {
        verifier,
        challenge,
    }
}

/// Generate an opaque `state` value for CSRF protection.
pub fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    B64URL.encode(bytes)
}

/// Register a public OAuth client with IBKR.
///
/// The registration endpoint is open, so a deployment can provision its own
/// `client_id` without a partner agreement. IBKR grants the full MCP scope set
/// regardless of what is requested; the narrower scope is applied at
/// authorization time instead.
pub async fn register_client(
    http: &reqwest::Client,
    client_name: &str,
    redirect_uri: &str,
) -> Result<String> {
    let body = serde_json::json!({
        "client_name": client_name,
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
        "scope": IBKR_SCOPE,
    });

    let response = http
        .post(IBKR_REGISTER_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Unexpected(format!("IBKR client registration request failed: {e}")))?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        Error::Unexpected(format!(
            "IBKR client registration returned an unreadable body (HTTP {status}): {e}"
        ))
    })?;

    let parsed: RegistrationResponse = serde_json::from_str(&body).map_err(|_| {
        Error::Unexpected(format!(
            "IBKR client registration returned a non-JSON body (HTTP {status}): {}",
            truncate(&body, 300)
        ))
    })?;

    parsed.client_id.ok_or_else(|| {
        Error::Unexpected(format!(
            "IBKR client registration failed (HTTP {status}): {} {}",
            parsed.error.unwrap_or_default(),
            parsed.error_description.unwrap_or_default()
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

/// Build the URL the user must visit to grant access.
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("scope", IBKR_SCOPE),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("resource", IBKR_MCP_RESOURCE),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{IBKR_AUTHORIZE_URL}?{query}")
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

struct CachedAccessToken {
    token: String,
    expires_at: DateTime<Utc>,
}

/// Owns the IBKR token lifecycle: exchanges authorization codes, caches the
/// short-lived access token in memory, and persists rotated refresh tokens.
pub struct IbkrTokenManager {
    http: reqwest::Client,
    client_id: String,
    store: Arc<dyn IbkrTokenStore>,
    cached: Mutex<Option<CachedAccessToken>>,
}

impl IbkrTokenManager {
    pub fn new(http: reqwest::Client, client_id: String, store: Arc<dyn IbkrTokenStore>) -> Self {
        Self {
            http,
            client_id,
            store,
            cached: Mutex::new(None),
        }
    }

    /// Exchange an authorization code for tokens and persist the refresh token.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<()> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", self.client_id.as_str()),
            ("code_verifier", verifier),
            ("resource", IBKR_MCP_RESOURCE),
        ];
        let token = self.post_token(&params).await?;

        let refresh = token.refresh_token.ok_or_else(|| {
            Error::Unexpected(
                "IBKR did not return a refresh token; unattended sync is not possible".into(),
            )
        })?;
        self.store.save_refresh_token(&refresh)?;
        self.cache(token.access_token, token.expires_in).await?;
        Ok(())
    }

    /// Return a valid access token, refreshing it when needed.
    pub async fn access_token(&self) -> Result<String> {
        {
            let cached = self.cached.lock().await;
            if let Some(entry) = cached.as_ref() {
                if entry.expires_at > Utc::now() {
                    return Ok(entry.token.clone());
                }
            }
        }
        self.refresh().await
    }

    /// Force a refresh using the stored refresh token.
    pub async fn refresh(&self) -> Result<String> {
        let refresh_token = self.store.load_refresh_token()?.ok_or_else(|| {
            Error::Unexpected("IBKR is not connected: no refresh token stored".into())
        })?;

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", self.client_id.as_str()),
            ("resource", IBKR_MCP_RESOURCE),
        ];
        let token = self.post_token(&params).await?;

        // IBKR rotates the refresh token: losing the new one would strand the
        // integration and force another human consent.
        if let Some(new_refresh) = token.refresh_token.as_deref() {
            if new_refresh != refresh_token {
                self.store.save_refresh_token(new_refresh)?;
            }
        }
        self.cache(token.access_token, token.expires_in).await
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.store.load_refresh_token(), Ok(Some(_)))
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.store.clear_refresh_token()?;
        *self.cached.lock().await = None;
        Ok(())
    }

    async fn cache(&self, access_token: Option<String>, expires_in: Option<i64>) -> Result<String> {
        let token = access_token.ok_or_else(|| {
            Error::Unexpected("IBKR token response did not include an access token".into())
        })?;
        let lifetime = expires_in.unwrap_or(600).max(EXPIRY_SKEW_SECONDS + 1);
        *self.cached.lock().await = Some(CachedAccessToken {
            token: token.clone(),
            expires_at: Utc::now() + Duration::seconds(lifetime - EXPIRY_SKEW_SECONDS),
        });
        Ok(token)
    }

    async fn post_token(&self, params: &[(&str, &str)]) -> Result<TokenResponse> {
        let response = self
            .http
            .post(IBKR_TOKEN_URL)
            .form(params)
            .send()
            .await
            .map_err(|e| Error::Unexpected(format!("IBKR token request failed: {e}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            Error::Unexpected(format!(
                "IBKR token endpoint returned an unreadable body (HTTP {status}): {e}"
            ))
        })?;

        let parsed: TokenResponse = serde_json::from_str(&body).map_err(|_| {
            Error::Unexpected(format!(
                "IBKR token endpoint returned a non-JSON body (HTTP {status}): {}",
                truncate(&body, 300)
            ))
        })?;

        if parsed.access_token.is_none() {
            return Err(Error::Unexpected(format!(
                "IBKR token request rejected (HTTP {status}): {} {}",
                parsed.error.clone().unwrap_or_default(),
                parsed.error_description.clone().unwrap_or_default()
            )));
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let pair = generate_pkce();
        let expected = B64URL.encode(Sha256::digest(pair.verifier.as_bytes()));
        assert_eq!(pair.challenge, expected);
        assert!(!pair.verifier.contains('='));
    }

    #[test]
    fn authorize_url_requests_read_only_scope_and_resource() {
        let url = build_authorize_url("cid", "http://localhost:8080/callback", "chal", "st");
        assert!(url.contains("scope=mcp.read"));
        assert!(!url.contains("orders.submit"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback"));
        assert!(url.contains("resource=https%3A%2F%2Fapi.ibkr.com%2Fv1%2Fapi%2Fmcp"));
    }
}
