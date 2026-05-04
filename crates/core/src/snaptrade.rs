//! SnapTrade Direct API integration module.
//!
//! This module provides types and functions for directly integrating with
//! the SnapTrade API using HMAC-SHA256 signed requests.
//!
//! API Base URL: https://api.snaptrade.com/api/v1

use crate::errors::Error;
use hmac::{Hmac, Mac};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::BTreeMap;

/// SnapTrade API base URL.
pub const SNAPTRADE_API_BASE: &str = "https://api.snaptrade.com/api/v1";

// ============================================================================
// CONFIGURATION
// ============================================================================

/// Configuration for SnapTrade API authentication.
#[derive(Debug, Clone)]
pub struct SnapTradeConfig {
    pub client_id: String,
    pub consumer_key: String,
}

impl SnapTradeConfig {
    /// Create config from environment variables.
    ///
    /// Reads:
    /// - SNAPTRADE_CLIENT_ID (required)
    /// - SNAPTRADE_CONSUMER_KEY (required)
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("SNAPTRADE_CLIENT_ID").ok()?;
        let consumer_key = std::env::var("SNAPTRADE_CONSUMER_KEY").ok()?;

        if client_id.is_empty() || consumer_key.is_empty() {
            return None;
        }

        Some(Self {
            client_id,
            consumer_key,
        })
    }
}

// ============================================================================
// MODELS
// ============================================================================

/// Response from registering a new SnapTrade user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeUserSecret {
    pub user_id: String,
    pub user_secret: String,
}

/// Brokerage information returned by SnapTrade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeBrokerage {
    pub id: Option<String>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub display_name: Option<String>,
}

/// A brokerage connection/authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeConnection {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub connection_type: Option<String>,
    pub brokerage: Option<SnapTradeBrokerage>,
    pub created_date: Option<String>,
    pub updated_date: Option<String>,
    pub disabled: Option<bool>,
}

impl SnapTradeConnection {
    /// Get the brokerage/institution name from the nested brokerage object.
    pub fn institution_name(&self) -> Option<&str> {
        self.brokerage.as_ref().and_then(|b| b.name.as_deref())
    }
}

/// A brokerage account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeAccount {
    pub id: String,
    pub name: Option<String>,
    pub number: Option<String>,
    pub institution_name: Option<String>,
    pub currency: Option<SnapTradeCurrency>,
    pub balance: Option<SnapTradeBalance>,
    pub sync_status: Option<SnapTradeSyncStatus>,
}

/// Account balance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeBalance {
    pub total: Option<SnapTradeAmount>,
    pub cash: Option<SnapTradeAmount>,
}

/// Amount with currency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeAmount {
    pub amount: Option<f64>,
    pub currency: Option<String>,
}

/// Sync status for an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeSyncStatus {
    pub transactions: Option<SnapTradeSyncStatusDetail>,
    pub holdings: Option<SnapTradeSyncStatusDetail>,
}

/// Detailed sync status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeSyncStatusDetail {
    pub initial_sync_completed: Option<bool>,
    pub last_successful_sync: Option<String>,
}

/// Holdings for an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeAccountHoldings {
    pub account: Option<SnapTradeAccount>,
    pub positions: Option<Vec<SnapTradePosition>>,
    pub balances: Option<Vec<SnapTradeHoldingBalance>>,
    pub total_value: Option<SnapTradeHoldingTotalValue>,
}

/// Balance entry in holdings response (different format from account balance).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SnapTradeHoldingBalance {
    pub currency: Option<SnapTradeCurrency>,
    pub cash: Option<f64>,
    pub buying_power: Option<f64>,
}

/// Total value in holdings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SnapTradeHoldingTotalValue {
    pub currency: Option<String>,
    pub value: Option<f64>,
}

/// A position/holding in an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradePosition {
    pub symbol: Option<SnapTradePositionSymbol>,
    pub units: Option<f64>,
    pub price: Option<f64>,
    pub open_pnl: Option<f64>,
    pub fractional_units: Option<f64>,
    pub average_purchase_price: Option<f64>,
    pub currency: Option<SnapTradeCurrency>,
}

/// Position symbol wrapper (contains nested symbol details).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradePositionSymbol {
    pub id: Option<String>,
    pub symbol: Option<SnapTradeSymbol>,
    pub description: Option<String>,
    pub local_id: Option<String>,
    pub security_type: Option<serde_json::Value>,
    pub listing_exchange: Option<serde_json::Value>,
    pub is_quotable: Option<bool>,
    pub is_tradable: Option<bool>,
}

/// Symbol information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeSymbol {
    pub id: Option<String>,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub currency: Option<SnapTradeCurrency>,
    pub exchange: Option<SnapTradeExchange>,
}

/// Currency information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeCurrency {
    pub id: Option<String>,
    pub code: Option<String>,
    pub name: Option<String>,
}

/// Exchange information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeExchange {
    pub id: Option<String>,
    pub code: Option<String>,
    pub name: Option<String>,
}

/// A transaction/activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeActivity {
    pub id: Option<String>,
    pub trade_date: Option<String>,
    pub settlement_date: Option<String>,
    #[serde(alias = "type")]
    pub action: Option<String>,
    pub symbol: Option<SnapTradeSymbol>,
    pub price: Option<f64>,
    pub units: Option<f64>,
    pub amount: Option<f64>,
    pub currency: Option<SnapTradeCurrency>,
    pub description: Option<String>,
    pub account: Option<SnapTradeAccountRef>,
    pub external_reference_id: Option<String>,
}

/// Reference to an account (used in activities).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeAccountRef {
    pub id: Option<String>,
    pub name: Option<String>,
    pub number: Option<String>,
    pub brokerage_authorization: Option<String>,
}

/// Login URL response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTradeLoginResponse {
    pub redirect_uri: Option<String>,
    #[serde(alias = "redirectURI")]
    pub redirect_uri_alt: Option<String>,
}

impl SnapTradeLoginResponse {
    pub fn get_redirect_uri(&self) -> Option<&str> {
        self.redirect_uri
            .as_deref()
            .or(self.redirect_uri_alt.as_deref())
    }
}

// ============================================================================
// SIGNATURE GENERATION
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Build HMAC-SHA256 signature for SnapTrade API request.
///
/// `body` is the raw JSON value (object/array), NOT a stringified JSON string.
/// SnapTrade expects `content` to be the actual body object in the signature payload,
/// e.g. `{"content":{"userId":"x"},...}` NOT `{"content":"{\"userId\":\"x\"}",...}`.
///
/// The `path` may contain query params (e.g., `/activities?startDate=2024-01-01`).
/// These are extracted and merged with auth params. The signature uses just the base path.
///
/// Returns (base_path, query_string, sig_input, base64_signature)
pub fn build_signature(
    cfg: &SnapTradeConfig,
    path: &str,
    body: Option<&serde_json::Value>,
    user_id: Option<&str>,
    user_secret: Option<&str>,
) -> Result<(String, String, String, String), Error> {
    if cfg.consumer_key.is_empty() {
        return Err(Error::Unexpected(
            "SnapTrade consumer_key is empty".to_string(),
        ));
    }

    // Split path into base path and any existing query params
    let (base_path, existing_query) = if let Some(idx) = path.find('?') {
        (&path[..idx], Some(&path[idx + 1..]))
    } else {
        (path, None)
    };

    // Build sorted query params (auth params + any existing params from path)
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    params.insert("clientId".to_string(), cfg.client_id.clone());
    params.insert(
        "timestamp".to_string(),
        chrono::Utc::now().timestamp().to_string(),
    );

    if let Some(uid) = user_id {
        params.insert("userId".to_string(), uid.to_string());
    }
    if let Some(secret) = user_secret {
        params.insert("userSecret".to_string(), secret.to_string());
    }

    // Parse and merge existing query params from path
    if let Some(existing) = existing_query {
        for pair in existing.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                params.insert(k.to_string(), v.to_string());
            }
        }
    }

    // Build query string (sorted, URL-encoded)
    let query_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Build content: raw body object, or null for no-body requests (matches TypeScript SDK)
    let content: serde_json::Value = body.cloned().unwrap_or(serde_json::Value::Null);

    // Build signature input JSON with ALPHABETICALLY SORTED keys (required by SnapTrade)
    // Must be exactly: {"content":...,"path":...,"query":...}
    // Using BTreeMap to guarantee alphabetical key order
    // NOTE: path in signature is JUST the base path, without query params
    let mut sig_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    sig_map.insert("content".to_string(), content);
    sig_map.insert("path".to_string(), serde_json::Value::String(format!("/api/v1{}", base_path)));
    sig_map.insert("query".to_string(), serde_json::Value::String(query_string.clone()));
    
    let sig_input_str = serde_json::to_string(&sig_map)
        .map_err(|e| Error::Unexpected(format!("Failed to serialize sig input: {}", e)))?;

    info!("SnapTrade sig_input: {}", sig_input_str);

    // TypeScript SDK uses encodeURI(consumerKey) - but in practice this is usually a no-op
    // for alphanumeric keys. Use raw UTF-8 bytes.
    let key_bytes = cfg.consumer_key.as_bytes().to_vec();

    // HMAC-SHA256
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .map_err(|e| Error::Unexpected(format!("HMAC key error: {}", e)))?;
    mac.update(sig_input_str.as_bytes());
    let result = mac.finalize();
    let signature = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        result.into_bytes(),
    );

    info!("SnapTrade signature: {}", signature);

    Ok((base_path.to_string(), query_string, sig_input_str, signature))
}

// ============================================================================
// HTTP HELPERS
// ============================================================================

async fn snaptrade_request(
    method: reqwest::Method,
    cfg: &SnapTradeConfig,
    path: &str,
    body: Option<serde_json::Value>,
    user_id: Option<&str>,
    user_secret: Option<&str>,
) -> Result<reqwest::Response, Error> {
    let (base_path, query_string, _sig_input, signature) =
        build_signature(cfg, path, body.as_ref(), user_id, user_secret)?;

    // Use base_path (without query params from input path) for the URL
    let url = format!("{}{}?{}", SNAPTRADE_API_BASE, base_path, query_string);
    debug!("SnapTrade {} {}", method, url);

    let client = reqwest::Client::new();
    let mut request = client.request(method, &url).header("Signature", signature);

    if let Some(b) = body {
        request = request
            .header("Content-Type", "application/json")
            .json(&b);
    }

    let response = request
        .send()
        .await
        .map_err(|e| Error::Unexpected(format!("SnapTrade request failed: {}", e)))?;

    Ok(response)
}

async fn snaptrade_get<T: for<'de> Deserialize<'de>>(
    cfg: &SnapTradeConfig,
    path: &str,
    user_id: Option<&str>,
    user_secret: Option<&str>,
) -> Result<T, Error> {
    let response =
        snaptrade_request(reqwest::Method::GET, cfg, path, None, user_id, user_secret).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Unexpected(format!(
            "SnapTrade API error {}: {}",
            status, body
        )));
    }

    // Log raw response for debugging
    let body_text = response.text().await.unwrap_or_default();
    info!("SnapTrade raw response: {}", body_text);
    
    serde_json::from_str(&body_text)
        .map_err(|e| Error::Unexpected(format!("Failed to parse SnapTrade response: {} - body: {}", e, body_text)))
}

async fn snaptrade_post<T: for<'de> Deserialize<'de>>(
    cfg: &SnapTradeConfig,
    path: &str,
    body: serde_json::Value,
    user_id: Option<&str>,
    user_secret: Option<&str>,
) -> Result<T, Error> {
    let response = snaptrade_request(
        reqwest::Method::POST,
        cfg,
        path,
        Some(body),
        user_id,
        user_secret,
    )
    .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Unexpected(format!(
            "SnapTrade API error {}: {}",
            status, body
        )));
    }

    response
        .json()
        .await
        .map_err(|e| Error::Unexpected(format!("Failed to parse SnapTrade response: {}", e)))
}

async fn snaptrade_delete(
    cfg: &SnapTradeConfig,
    path: &str,
    user_id: Option<&str>,
    user_secret: Option<&str>,
) -> Result<(), Error> {
    let response =
        snaptrade_request(reqwest::Method::DELETE, cfg, path, None, user_id, user_secret).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Unexpected(format!(
            "SnapTrade API error {}: {}",
            status, body
        )));
    }

    Ok(())
}

// ============================================================================
// PUBLIC API FUNCTIONS
// ============================================================================

/// Register a new SnapTrade user.
///
/// POST /snapTrade/registerUser
pub async fn register_user(cfg: &SnapTradeConfig, user_id: &str) -> Result<SnapTradeUserSecret, Error> {
    let body = serde_json::json!({ "userId": user_id });
    snaptrade_post(cfg, "/snapTrade/registerUser", body, None, None).await
}

/// Delete a SnapTrade user.
///
/// DELETE /snapTrade/deleteUser
pub async fn delete_snaptrade_user(cfg: &SnapTradeConfig, user_id: &str) -> Result<(), Error> {
    // Note: deleteUser requires userId in query params, not body
    snaptrade_delete(cfg, "/snapTrade/deleteUser", Some(user_id), None).await
}

/// Get SnapTrade login/connection URL.
///
/// POST /snapTrade/login
pub async fn get_login_url(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
    redirect_uri: &str,
) -> Result<String, Error> {
    let body = serde_json::json!({
        "userId": user_id,
        "userSecret": user_secret,
        "redirectURI": redirect_uri
    });
    let response: SnapTradeLoginResponse =
        snaptrade_post(cfg, "/snapTrade/login", body, Some(user_id), Some(user_secret)).await?;

    response
        .get_redirect_uri()
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Unexpected("No redirect URI in login response".to_string()))
}

/// List all brokerage connections/authorizations for a user.
///
/// GET /authorizations
pub async fn list_connections(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
) -> Result<Vec<SnapTradeConnection>, Error> {
    snaptrade_get(cfg, "/authorizations", Some(user_id), Some(user_secret)).await
}

/// Delete a specific brokerage connection.
///
/// DELETE /authorizations/{authorizationId}
pub async fn delete_connection(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
    auth_id: &str,
) -> Result<(), Error> {
    let path = format!("/authorizations/{}", auth_id);
    snaptrade_delete(cfg, &path, Some(user_id), Some(user_secret)).await
}

/// List all accounts for a user.
///
/// GET /accounts
pub async fn list_accounts(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
) -> Result<Vec<SnapTradeAccount>, Error> {
    snaptrade_get(cfg, "/accounts", Some(user_id), Some(user_secret)).await
}

/// List holdings for a user (optionally filtered by account).
///
/// GET /holdings or GET /accounts/{accountId}/holdings
pub async fn list_holdings(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
    account_id: Option<&str>,
) -> Result<Vec<SnapTradeAccountHoldings>, Error> {
    let path = match account_id {
        Some(aid) => format!("/accounts/{}/holdings", aid),
        None => "/holdings".to_string(),
    };
    snaptrade_get(cfg, &path, Some(user_id), Some(user_secret)).await
}

/// List activities/transactions for a user.
///
/// GET /activities
pub async fn list_activities(
    cfg: &SnapTradeConfig,
    user_id: &str,
    user_secret: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<Vec<SnapTradeActivity>, Error> {
    let mut path = "/activities".to_string();
    let mut has_param = false;

    if let Some(sd) = start_date {
        path.push_str(&format!("{}startDate={}", if has_param { "&" } else { "?" }, sd));
        has_param = true;
    }
    if let Some(ed) = end_date {
        path.push_str(&format!("{}endDate={}", if has_param { "&" } else { "?" }, ed));
    }

    snaptrade_get(cfg, &path, Some(user_id), Some(user_secret)).await
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Map SnapTrade action to WF activity type.
pub fn map_snaptrade_action(action: &str) -> &'static str {
    match action.to_uppercase().as_str() {
        "BUY" => "BUY",
        "SELL" => "SELL",
        "DIV" | "DIVIDEND" => "DIVIDEND",
        "INTEREST" => "INTEREST",
        "DEPOSIT" => "DEPOSIT",
        "WITHDRAWAL" => "WITHDRAWAL",
        "FEE" => "FEE",
        "TRANSFER" | "TRANSFER_IN" => "TRANSFER_IN",
        "TRANSFER_OUT" => "TRANSFER_OUT",
        "SPLIT" => "SPLIT",
        _ => "OTHER",
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SnapTradeConfig {
        SnapTradeConfig {
            client_id: "test-client-id".to_string(),
            consumer_key: "test-consumer-key-12345".to_string(),
        }
    }

    #[test]
    fn test_signature_empty_body() {
        let cfg = test_config();
        let result = build_signature(&cfg, "/accounts", None, None, None);
        assert!(result.is_ok());
        let (_base_path, query_string, _sig_input, signature) = result.unwrap();

        assert!(query_string.contains("clientId=test-client-id"));
        assert!(query_string.contains("timestamp="));
        assert!(!signature.is_empty());
        // Signature should be base64 encoded
        assert!(base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &signature
        ).is_ok());
    }

    #[test]
    fn test_signature_with_body() {
        let cfg = test_config();
        let body = serde_json::json!({"userId": "test-user"});
        let result = build_signature(&cfg, "/snapTrade/registerUser", Some(&body), None, None);
        assert!(result.is_ok());
        let (_, _, sig_input, signature) = result.unwrap();
        assert!(!signature.is_empty());
        // Verify content is raw object, NOT stringified
        assert!(sig_input.contains(r#""content":{"userId":"test-user"}"#));
        assert!(!sig_input.contains(r#""content":"{\"userId"#));
    }

    #[test]
    fn test_query_string_sorted() {
        let cfg = test_config();
        let result = build_signature(
            &cfg,
            "/accounts",
            None,
            Some("zuser"),
            Some("asecret"),
        );
        assert!(result.is_ok());
        let (_, query_string, _, _) = result.unwrap();
        
        // Keys should be sorted alphabetically: clientId, timestamp, userId, userSecret
        let parts: Vec<&str> = query_string.split('&').collect();
        assert!(parts.len() == 4);
        assert!(parts[0].starts_with("clientId="));
        assert!(parts[1].starts_with("timestamp="));
        assert!(parts[2].starts_with("userId="));
        assert!(parts[3].starts_with("userSecret="));
    }

    #[test]
    fn test_signature_with_user_params() {
        let cfg = test_config();
        let result = build_signature(
            &cfg,
            "/authorizations",
            None,
            Some("my-user-id"),
            Some("my-user-secret"),
        );
        assert!(result.is_ok());
        let (_, query_string, _, signature) = result.unwrap();

        assert!(query_string.contains("userId=my-user-id"));
        assert!(query_string.contains("userSecret=my-user-secret"));
        assert!(!signature.is_empty());
    }

    #[test]
    fn test_snaptrade_config_missing_key() {
        let cfg = SnapTradeConfig {
            client_id: "test".to_string(),
            consumer_key: String::new(),
        };
        let result = build_signature(&cfg, "/test", None, None, None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("consumer_key"));
    }

    #[test]
    fn test_path_with_query_params() {
        let cfg = test_config();
        // Path with existing query params (like list_activities uses)
        let result = build_signature(
            &cfg,
            "/activities?startDate=2024-01-01&endDate=2026-03-28",
            None,
            Some("user123"),
            Some("secret456"),
        );
        assert!(result.is_ok());
        let (base_path, query_string, sig_input, _signature) = result.unwrap();
        
        // Base path should NOT contain query params
        assert_eq!(base_path, "/activities");
        assert!(!base_path.contains('?'));
        
        // Query string should contain BOTH auth params AND date params (all sorted)
        assert!(query_string.contains("clientId="));
        assert!(query_string.contains("userId=user123"));
        assert!(query_string.contains("userSecret=secret456"));
        assert!(query_string.contains("startDate=2024-01-01"));
        assert!(query_string.contains("endDate=2026-03-28"));
        
        // Signature input path should be just the base path with /api/v1 prefix
        // Query params are in the "query" field, NOT in the "path" field
        assert!(sig_input.contains(r#""path":"/api/v1/activities""#));
        // Verify path in signature doesn't contain query params
        assert!(!sig_input.contains(r#""path":"/api/v1/activities?"#));
    }

    #[test]
    fn test_register_user_serialization() {
        let json = r#"{
            "userId": "dfc-user-abc12",
            "userSecret": "secret-xyz-789"
        }"#;
        let result: Result<SnapTradeUserSecret, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.user_id, "dfc-user-abc12");
        assert_eq!(user.user_secret, "secret-xyz-789");
    }

    #[test]
    fn test_activity_deserialization() {
        // Test with partial/null fields - should not panic
        let json = r#"{
            "id": "act-123",
            "trade_date": "2026-03-19",
            "action": "BUY",
            "units": 10.0
        }"#;
        let result: Result<SnapTradeActivity, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let activity = result.unwrap();
        assert_eq!(activity.id, Some("act-123".to_string()));
        assert!(activity.price.is_none());
        assert!(activity.symbol.is_none());

        // Test with all null/missing fields
        let json_minimal = r#"{}"#;
        let result_minimal: Result<SnapTradeActivity, _> = serde_json::from_str(json_minimal);
        assert!(result_minimal.is_ok());
    }

    #[test]
    fn test_get_login_url_extracts_redirect_uri() {
        // Test with redirectUri field
        let json1 = r#"{"redirectUri": "https://connect.snaptrade.com/abc123"}"#;
        let response1: SnapTradeLoginResponse = serde_json::from_str(json1).unwrap();
        assert_eq!(
            response1.get_redirect_uri(),
            Some("https://connect.snaptrade.com/abc123")
        );

        // Test with redirectURI field (alternative casing)
        let json2 = r#"{"redirectURI": "https://connect.snaptrade.com/xyz789"}"#;
        let response2: SnapTradeLoginResponse = serde_json::from_str(json2).unwrap();
        assert_eq!(
            response2.get_redirect_uri(),
            Some("https://connect.snaptrade.com/xyz789")
        );

        // Test with empty response
        let json3 = r#"{}"#;
        let response3: SnapTradeLoginResponse = serde_json::from_str(json3).unwrap();
        assert!(response3.get_redirect_uri().is_none());
    }

    #[test]
    fn test_map_snaptrade_action() {
        assert_eq!(map_snaptrade_action("BUY"), "BUY");
        assert_eq!(map_snaptrade_action("buy"), "BUY");
        assert_eq!(map_snaptrade_action("SELL"), "SELL");
        assert_eq!(map_snaptrade_action("DIV"), "DIVIDEND");
        assert_eq!(map_snaptrade_action("DIVIDEND"), "DIVIDEND");
        assert_eq!(map_snaptrade_action("INTEREST"), "INTEREST");
        assert_eq!(map_snaptrade_action("DEPOSIT"), "DEPOSIT");
        assert_eq!(map_snaptrade_action("WITHDRAWAL"), "WITHDRAWAL");
        assert_eq!(map_snaptrade_action("FEE"), "FEE");
        assert_eq!(map_snaptrade_action("TRANSFER"), "TRANSFER_IN");
        assert_eq!(map_snaptrade_action("UNKNOWN"), "OTHER");
    }

    #[test]
    fn test_account_deserialization() {
        let json = r#"{
            "id": "acc-123",
            "name": "Brokerage Account",
            "number": "12345678",
            "institution_name": "Fidelity",
            "currency": { "code": "USD" },
            "balance": { "total": { "amount": 10000.50, "currency": "USD" } }
        }"#;
        let result: Result<SnapTradeAccount, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let account = result.unwrap();
        assert_eq!(account.id, "acc-123");
        assert_eq!(account.name, Some("Brokerage Account".to_string()));
    }

    #[test]
    fn test_connection_deserialization() {
        let json = r#"{
            "id": "conn-456",
            "name": "My Fidelity",
            "type": "read",
            "brokerage": {
                "id": "brokerage-123",
                "name": "Fidelity",
                "slug": "FIDELITY",
                "display_name": "Fidelity Investments"
            },
            "created_date": "2026-01-15",
            "updated_date": "2026-01-16",
            "disabled": false
        }"#;
        let result: Result<SnapTradeConnection, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let conn = result.unwrap();
        assert_eq!(conn.id, "conn-456");
        // brokerage is now a nested object - use the helper method
        assert_eq!(conn.institution_name(), Some("Fidelity"));
    }
}
