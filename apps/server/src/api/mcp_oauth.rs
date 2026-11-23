//! OAuth 2.1 authorization for the MCP endpoint, as required by remote MCP
//! clients such as claude.ai custom connectors.
//!
//! - Discovery: RFC 9728 protected-resource and RFC 8414 server metadata.
//! - Dynamic client registration (RFC 7591) is stateless: the client id
//!   encodes its name and redirect URIs (registration is open anyway, so a
//!   signature would add nothing).
//! - Authorization code + PKCE (S256): the consent page lives in the Orus app,
//!   where the logged-in user picks the READ or WRITE scope.
//! - The access token is a regular MCP API key, revocable in the settings.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wealthfolio_storage_postgres::mcp::NewMcpApiKey;

use super::mcp::{generate_secret, hash_secret, key_repository, random_hex, SCOPE_READ, SCOPE_WRITE};
use crate::{
    error::{ApiError, ApiResult},
    main_lib::AppState,
};

const CLIENT_PREFIX: &str = "orusc_";
const CODE_TTL_MINUTES: i64 = 5;
const MCP_PATH: &str = "/api/v1/mcp";

/// Discovery documents, served at the origin root.
pub fn well_known_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/.well-known/oauth-protected-resource", get(protected_resource_metadata))
        .route("/.well-known/oauth-protected-resource/{*resource}", get(protected_resource_metadata))
        .route("/.well-known/oauth-authorization-server", get(authorization_server_metadata))
        .route(
            "/.well-known/oauth-authorization-server/{*issuer}",
            get(authorization_server_metadata),
        )
}

/// Public endpoints called by the MCP client itself.
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/mcp/oauth/register", post(register))
        .route("/mcp/oauth/token", post(token))
}

/// Endpoints used by the consent page, mounted under the JWT-protected API.
pub fn consent_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/mcp/oauth/client", get(describe_client))
        .route("/mcp/oauth/approve", post(approve))
}

// ---------------------------------------------------------------------------
// URLs
// ---------------------------------------------------------------------------

/// Public origin of this server, as seen by the client.
pub fn public_base_url(headers: &HeaderMap) -> String {
    if let Some(url) = env_url("WF_PUBLIC_API_URL") {
        return url;
    }
    let first = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let host = first("x-forwarded-host")
        .or_else(|| first(header::HOST.as_str()))
        .unwrap_or_else(|| "localhost:8080".to_string());
    let local = host.starts_with("localhost") || host.starts_with("127.0.0.1");
    let proto = first("x-forwarded-proto").unwrap_or_else(|| if local { "http" } else { "https" }.into());
    format!("{proto}://{host}")
}

/// Origin of the Orus web app hosting the consent page.
fn public_app_url(base: &str) -> String {
    env_url("WF_PUBLIC_APP_URL")
        .or_else(|| {
            std::env::var("WF_CORS_ALLOW_ORIGINS").ok().and_then(|origins| {
                origins
                    .split(',')
                    .map(|o| o.trim().trim_end_matches('/'))
                    .find(|o| o.starts_with("https://"))
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| base.to_string())
}

fn env_url(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

pub fn resource_metadata_url(headers: &HeaderMap) -> String {
    format!("{}/.well-known/oauth-protected-resource", public_base_url(headers))
}

async fn protected_resource_metadata(headers: HeaderMap) -> Json<Value> {
    let base = public_base_url(&headers);
    Json(json!({
        "resource": format!("{base}{MCP_PATH}"),
        "authorization_servers": [base],
        "scopes_supported": [SCOPE_READ, SCOPE_WRITE],
        "bearer_methods_supported": ["header"],
        "resource_name": "Orus",
    }))
}

async fn authorization_server_metadata(headers: HeaderMap) -> Json<Value> {
    let base = public_base_url(&headers);
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{}/oauth/authorize", public_app_url(&base)),
        "token_endpoint": format!("{base}{MCP_PATH}/oauth/token"),
        "registration_endpoint": format!("{base}{MCP_PATH}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": [SCOPE_READ, SCOPE_WRITE],
    }))
}

// ---------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ClientInfo {
    #[serde(rename = "n")]
    name: String,
    #[serde(rename = "r")]
    redirect_uris: Vec<String>,
}

fn encode_client(info: &ClientInfo) -> String {
    let payload = serde_json::to_vec(info).unwrap_or_default();
    format!("{CLIENT_PREFIX}{}", URL_SAFE_NO_PAD.encode(payload))
}

fn decode_client(client_id: &str) -> Option<ClientInfo> {
    let payload = URL_SAFE_NO_PAD.decode(client_id.strip_prefix(CLIENT_PREFIX)?).ok()?;
    serde_json::from_slice(&payload).ok()
}

fn is_valid_redirect_uri(uri: &str) -> bool {
    if uri.contains('#') || uri.chars().any(char::is_whitespace) {
        return false;
    }
    let loopback = ["http://localhost", "http://127.0.0.1", "http://[::1]"];
    uri.starts_with("https://") || loopback.iter().any(|prefix| uri.starts_with(prefix))
}

/// The client and redirect URI of an authorization request, once validated.
fn validated_client(client_id: &str, redirect_uri: &str) -> ApiResult<ClientInfo> {
    let client = decode_client(client_id)
        .ok_or_else(|| ApiError::BadRequest("Unknown OAuth client".to_string()))?;
    if !client.redirect_uris.iter().any(|uri| uri == redirect_uri) {
        return Err(ApiError::BadRequest(
            "redirect_uri is not registered for this client".to_string(),
        ));
    }
    Ok(client)
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({ "error": error, "error_description": description })),
    )
        .into_response()
}

#[derive(Deserialize)]
struct RegisterRequest {
    #[serde(default)]
    redirect_uris: Vec<String>,
    client_name: Option<String>,
}

async fn register(Json(request): Json<RegisterRequest>) -> Response {
    if request.redirect_uris.is_empty()
        || !request.redirect_uris.iter().all(|uri| is_valid_redirect_uri(uri))
    {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "redirect_uris must be https (or http loopback) URLs",
        );
    }
    let name: String = request
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Agent IA")
        .chars()
        .take(80)
        .collect();
    let client = ClientInfo { name, redirect_uris: request.redirect_uris };
    (
        StatusCode::CREATED,
        Json(json!({
            "client_id": encode_client(&client),
            "client_id_issued_at": Utc::now().timestamp(),
            "client_name": client.name,
            "redirect_uris": client.redirect_uris,
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Consent (JWT-protected, called by the Orus app)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DescribeQuery {
    client_id: String,
    redirect_uri: String,
}

async fn describe_client(Query(query): Query<DescribeQuery>) -> ApiResult<Json<Value>> {
    let client = validated_client(&query.client_id, &query.redirect_uri)?;
    Ok(Json(json!({ "clientName": client.name })))
}

struct PendingCode {
    client_id: String,
    client_name: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    expires_at: DateTime<Utc>,
}

fn pending_codes() -> &'static Mutex<HashMap<String, PendingCode>> {
    static CODES: OnceLock<Mutex<HashMap<String, PendingCode>>> = OnceLock::new();
    CODES.get_or_init(Default::default)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApproveRequest {
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: Option<String>,
    state: Option<String>,
    scope: String,
}

async fn approve(Json(request): Json<ApproveRequest>) -> ApiResult<Json<Value>> {
    let client = validated_client(&request.client_id, &request.redirect_uri)?;
    if request.code_challenge_method.as_deref() != Some("S256") {
        return Err(ApiError::BadRequest("PKCE with S256 is required".to_string()));
    }
    if !(43..=128).contains(&request.code_challenge.len()) {
        return Err(ApiError::BadRequest("Invalid code_challenge".to_string()));
    }
    let scope = request.scope.to_uppercase();
    if scope != SCOPE_READ && scope != SCOPE_WRITE {
        return Err(ApiError::BadRequest("Scope must be READ or WRITE".to_string()));
    }

    let code = random_hex(32);
    {
        let mut codes = pending_codes().lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now();
        codes.retain(|_, pending| pending.expires_at > now);
        codes.insert(
            code.clone(),
            PendingCode {
                client_id: request.client_id,
                client_name: client.name,
                redirect_uri: request.redirect_uri.clone(),
                code_challenge: request.code_challenge,
                scope,
                expires_at: now + Duration::minutes(CODE_TTL_MINUTES),
            },
        );
    }

    let separator = if request.redirect_uri.contains('?') { '&' } else { '?' };
    let mut redirect_url =
        format!("{}{separator}code={}", request.redirect_uri, urlencoding::encode(&code));
    if let Some(state) = request.state.filter(|s| !s.is_empty()) {
        redirect_url.push_str(&format!("&state={}", urlencoding::encode(&state)));
    }
    Ok(Json(json!({ "redirectUrl": redirect_url })))
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TokenRequest {
    grant_type: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    code_verifier: Option<String>,
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

async fn token(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    let parsed: Result<TokenRequest, String> = if is_json {
        serde_json::from_slice(&body).map_err(|e| e.to_string())
    } else {
        serde_urlencoded::from_bytes(&body).map_err(|e| e.to_string())
    };
    let request = match parsed {
        Ok(request) => request,
        Err(e) => return oauth_error(StatusCode::BAD_REQUEST, "invalid_request", &e),
    };
    if request.grant_type != "authorization_code" {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "Only authorization_code is supported",
        );
    }

    let pending = request
        .code
        .as_deref()
        .and_then(|code| pending_codes().lock().unwrap_or_else(|e| e.into_inner()).remove(code));
    let Some(pending) = pending.filter(|p| p.expires_at > Utc::now()) else {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", "Unknown or expired code");
    };
    if request.client_id.as_deref() != Some(pending.client_id.as_str())
        || request.redirect_uri.as_deref() != Some(pending.redirect_uri.as_str())
    {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", "Client or redirect_uri mismatch");
    }
    let verified = request
        .code_verifier
        .as_deref()
        .is_some_and(|verifier| pkce_challenge(verifier) == pending.code_challenge);
    if !verified {
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", "PKCE verification failed");
    }

    let Ok(repo) = key_repository(&state) else {
        return oauth_error(StatusCode::SERVICE_UNAVAILABLE, "server_error", "PostgreSQL storage required");
    };
    let secret = generate_secret();
    let created = repo.create_key(NewMcpApiKey {
        name: format!("OAuth · {}", pending.client_name),
        key_prefix: secret.chars().take(12).collect(),
        key_hash: hash_secret(&secret),
        scope: pending.scope.clone(),
    });
    if let Err(e) = created {
        tracing::error!("MCP OAuth key creation failed: {e}");
        return oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "Key creation failed");
    }

    let mut response = Json(json!({
        "access_token": secret,
        "token_type": "Bearer",
        "scope": pending.scope,
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_round_trips() {
        let client = ClientInfo {
            name: "Claude".into(),
            redirect_uris: vec!["https://claude.ai/api/mcp/auth_callback".into()],
        };
        let id = encode_client(&client);
        assert!(id.starts_with(CLIENT_PREFIX));
        assert_eq!(decode_client(&id), Some(client));
        assert_eq!(decode_client("orus_9db27c785ef1"), None);
    }

    #[test]
    fn redirect_uris_must_be_https_or_loopback() {
        assert!(is_valid_redirect_uri("https://claude.ai/api/mcp/auth_callback"));
        assert!(is_valid_redirect_uri("http://localhost:6274/callback"));
        assert!(!is_valid_redirect_uri("http://evil.example/callback"));
        assert!(!is_valid_redirect_uri("https://claude.ai/cb#frag"));
    }

    #[test]
    fn pkce_challenge_is_base64url_sha256() {
        // Reference computed with Node: createHash("sha256").update(v).digest("base64url")
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K9uvaW1Rf6ZQ6EPYcDhaZ2IHM0"),
            "UKv4CbJBgTQEAdxVlFhMUmK6aDbmL9e5paHtPpB5iU4"
        );
    }
}
