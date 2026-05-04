//! Revolut Business integration module - domain models for OAuth and sync.
//!
//! This module provides types for integrating with Revolut Business API:
//! - OAuth 2.0 token management
//! - Account synchronization
//! - Transaction import to WF activities
//!
//! API Base URL: https://b2b.revolut.com/api/1.0/

use crate::errors::Error;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use log::debug;
use serde::{Deserialize, Serialize};

// ============================================================================
// ENUMS
// ============================================================================

/// State of a Revolut account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RevolutAccountState {
    #[default]
    Active,
    Inactive,
    Blocked,
}

impl RevolutAccountState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Blocked => "blocked",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

/// State of a Revolut transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RevolutTransactionState {
    Pending,
    #[default]
    Completed,
    Declined,
    Failed,
    Reverted,
}

impl RevolutTransactionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Declined => "declined",
            Self::Failed => "failed",
            Self::Reverted => "reverted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "completed" => Some(Self::Completed),
            "declined" => Some(Self::Declined),
            "failed" => Some(Self::Failed),
            "reverted" => Some(Self::Reverted),
            _ => None,
        }
    }
}

/// Type of sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RevolutSyncType {
    #[default]
    Full,
    Incremental,
    AccountsOnly,
    TransactionsOnly,
}

impl RevolutSyncType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
            Self::AccountsOnly => "accounts_only",
            Self::TransactionsOnly => "transactions_only",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "full" => Some(Self::Full),
            "incremental" => Some(Self::Incremental),
            "accounts_only" => Some(Self::AccountsOnly),
            "transactions_only" => Some(Self::TransactionsOnly),
            _ => None,
        }
    }
}

/// Status of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RevolutSyncStatus {
    #[default]
    Started,
    Success,
    Partial,
    Error,
}

impl RevolutSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Success => "success",
            Self::Partial => "partial",
            Self::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "started" => Some(Self::Started),
            "success" => Some(Self::Success),
            "partial" => Some(Self::Partial),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

// ============================================================================
// OAUTH TOKENS
// ============================================================================

/// OAuth 2.0 token storage for Revolut Business API.
/// Access tokens should be encrypted at the application layer before storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevolutAuthToken {
    pub id: String,
    /// Encrypted access token
    pub access_token: String,
    /// Encrypted refresh token (optional, depends on OAuth flow)
    pub refresh_token: Option<String>,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// OAuth scopes granted
    pub scope: Option<String>,
    /// Token expiration timestamp (ISO 8601)
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Input model for storing a new OAuth token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRevolutAuthToken {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    /// Token lifetime in seconds (from OAuth response)
    pub expires_in: Option<i64>,
    /// Or explicit expiration timestamp
    pub expires_at: Option<String>,
}

// ============================================================================
// REVOLUT ACCOUNTS
// ============================================================================

/// Cached Revolut Business account.
/// Maps to a Wealthfolio account with type=CASH.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevolutAccount {
    pub id: String,
    /// Revolut's internal account ID
    pub revolut_account_id: String,
    pub name: String,
    pub currency: String,
    /// Balance as decimal string (SQLite TEXT)
    pub balance: String,
    pub state: String,
    /// Linked Wealthfolio account ID (if mapped)
    pub wf_account_id: Option<String>,
    pub last_synced_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl RevolutAccount {
    /// Parse the state string into an enum.
    pub fn state_enum(&self) -> Option<RevolutAccountState> {
        RevolutAccountState::from_str(&self.state)
    }

    /// Parse balance as f64.
    pub fn balance_f64(&self) -> Option<f64> {
        self.balance.parse().ok()
    }
}

/// Input for creating/updating a Revolut account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRevolutAccount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub revolut_account_id: String,
    pub name: String,
    pub currency: String,
    pub balance: String,
    pub state: Option<String>,
    pub wf_account_id: Option<String>,
}

// ============================================================================
// REVOLUT TRANSACTIONS
// ============================================================================

/// Revolut Business transaction.
/// Maps to Wealthfolio activities (DEPOSIT/WITHDRAWAL/FX).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevolutTransaction {
    pub id: String,
    /// Revolut's internal transaction ID
    pub revolut_transaction_id: String,
    /// Revolut account ID this transaction belongs to
    pub revolut_account_id: String,
    /// Transaction type from Revolut API
    pub r#type: String,
    pub state: String,
    /// Amount as decimal string
    pub amount: String,
    pub currency: String,
    pub description: Option<String>,
    pub merchant_name: Option<String>,
    pub merchant_category: Option<String>,
    pub reference: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    /// Balance after transaction
    pub balance_after: Option<String>,
    /// Linked Wealthfolio activity ID (if imported)
    pub wf_activity_id: Option<String>,
    /// Whether this tx has been synced to WF activities
    pub synced_to_activities: bool,
}

impl RevolutTransaction {
    /// Parse the state string into an enum.
    pub fn state_enum(&self) -> Option<RevolutTransactionState> {
        RevolutTransactionState::from_str(&self.state)
    }

    /// Parse amount as f64.
    pub fn amount_f64(&self) -> Option<f64> {
        self.amount.parse().ok()
    }

    /// Map Revolut transaction type to WF activity type.
    /// Returns (activity_type, is_positive_amount)
    pub fn map_to_wf_activity_type(&self) -> Option<(&'static str, bool)> {
        let amount = self.amount_f64()?;
        match self.r#type.to_uppercase().as_str() {
            "TRANSFER" => {
                if amount >= 0.0 {
                    Some(("DEPOSIT", true))
                } else {
                    Some(("WITHDRAWAL", false))
                }
            }
            "EXCHANGE" => Some(("FX", amount >= 0.0)),
            "CARD_PAYMENT" | "CARD_REFUND" => Some(("WITHDRAWAL", false)),
            "ATM" => Some(("WITHDRAWAL", false)),
            "FEE" => Some(("FEE", false)),
            "TOPUP" => Some(("DEPOSIT", true)),
            _ => None,
        }
    }
}

/// Input for creating a Revolut transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRevolutTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub revolut_transaction_id: String,
    pub revolut_account_id: String,
    pub r#type: String,
    pub state: String,
    pub amount: String,
    pub currency: String,
    pub description: Option<String>,
    pub merchant_name: Option<String>,
    pub merchant_category: Option<String>,
    pub reference: Option<String>,
    pub completed_at: Option<String>,
    pub balance_after: Option<String>,
}

// ============================================================================
// SYNC LOG
// ============================================================================

/// Audit log entry for Revolut sync operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevolutSyncLog {
    pub id: String,
    pub sync_type: String,
    pub status: String,
    pub accounts_synced: Option<i32>,
    pub transactions_synced: Option<i32>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

impl RevolutSyncLog {
    pub fn sync_type_enum(&self) -> Option<RevolutSyncType> {
        RevolutSyncType::from_str(&self.sync_type)
    }

    pub fn status_enum(&self) -> Option<RevolutSyncStatus> {
        RevolutSyncStatus::from_str(&self.status)
    }

    /// Check if sync completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self.status_enum(), Some(RevolutSyncStatus::Success))
    }

    /// Duration of the sync operation if completed.
    pub fn duration_ms(&self) -> Option<i64> {
        let start = chrono::DateTime::parse_from_rfc3339(&self.started_at).ok()?;
        let end = chrono::DateTime::parse_from_rfc3339(self.completed_at.as_ref()?).ok()?;
        Some((end - start).num_milliseconds())
    }
}

/// Input for creating a sync log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRevolutSyncLog {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub sync_type: String,
    pub status: String,
}

/// Update model for completing a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevolutSyncLogUpdate {
    pub status: String,
    pub accounts_synced: Option<i32>,
    pub transactions_synced: Option<i32>,
    pub error_message: Option<String>,
}

// ============================================================================
// API RESPONSE TYPES (for parsing Revolut API responses)
// ============================================================================

/// Revolut API account response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutApiAccount {
    pub id: String,
    pub name: String,
    pub balance: f64,
    pub currency: String,
    pub state: String,
    #[serde(default)]
    pub public: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<RevolutApiAccount> for NewRevolutAccount {
    fn from(api: RevolutApiAccount) -> Self {
        NewRevolutAccount {
            id: None,
            revolut_account_id: api.id,
            name: api.name,
            currency: api.currency,
            balance: api.balance.to_string(),
            state: Some(api.state),
            wf_account_id: None,
        }
    }
}

/// Revolut API transaction response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutApiTransaction {
    pub id: String,
    #[serde(rename = "type")]
    pub tx_type: String,
    pub state: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub reference: Option<String>,
    pub legs: Vec<RevolutApiTransactionLeg>,
    pub merchant: Option<RevolutApiMerchant>,
}

/// Transaction leg (amount for one currency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutApiTransactionLeg {
    pub leg_id: String,
    pub account_id: String,
    pub amount: f64,
    pub currency: String,
    pub description: Option<String>,
    pub balance: Option<f64>,
}

/// Merchant info for card transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutApiMerchant {
    pub name: Option<String>,
    pub category: Option<String>,
}

// ============================================================================
// OAUTH 2.0 CONFIGURATION & FLOW
// ============================================================================

/// Sandbox base URL for Revolut Business API.
pub const REVOLUT_SANDBOX_URL: &str = "https://sandbox-b2b.revolut.com/api/1.0";
/// Production base URL for Revolut Business API.
pub const REVOLUT_PRODUCTION_URL: &str = "https://b2b.revolut.com/api/1.0";

/// OAuth 2.0 configuration for Revolut Business API.
#[derive(Debug, Clone)]
pub struct RevolutOAuthConfig {
    pub client_id: String,
    pub redirect_uri: String,
    pub sandbox: bool,
    /// JWT issuer (iss claim) - typically a URL identifying your app.
    pub jwt_issuer: String,
    /// PEM-encoded private key for signing JWT client assertions (RS256).
    pub private_key_pem: String,
}

impl RevolutOAuthConfig {
    /// Create a new OAuth config from environment variables.
    ///
    /// Reads:
    /// - REVOLUT_CLIENT_ID (required)
    /// - REVOLUT_REDIRECT_URI (required)
    /// - REVOLUT_SANDBOX (optional, defaults to true)
    /// - REVOLUT_JWT_ISSUER (required for production)
    /// - REVOLUT_PRIVATE_KEY_PATH (required for production)
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("REVOLUT_CLIENT_ID").ok()?;
        let redirect_uri = std::env::var("REVOLUT_REDIRECT_URI").ok()?;
        let sandbox = std::env::var("REVOLUT_SANDBOX")
            .map(|v| v.to_lowercase() != "false")
            .unwrap_or(true);

        // JWT configuration for client assertion
        let jwt_issuer = std::env::var("REVOLUT_JWT_ISSUER").unwrap_or_default();
        let private_key_pem = std::env::var("REVOLUT_PRIVATE_KEY_PATH")
            .ok()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .unwrap_or_default();

        Some(Self {
            client_id,
            redirect_uri,
            sandbox,
            jwt_issuer,
            private_key_pem,
        })
    }

    /// Get the base URL for API calls.
    pub fn base_url(&self) -> &'static str {
        if self.sandbox {
            REVOLUT_SANDBOX_URL
        } else {
            REVOLUT_PRODUCTION_URL
        }
    }

    /// Get the authorization URL.
    pub fn authorize_url(&self) -> String {
        if self.sandbox {
            "https://sandbox-business.revolut.com/app-confirm".to_string()
        } else {
            "https://business.revolut.com/app-confirm".to_string()
        }
    }

    /// Build the full OAuth authorization URL with state parameter.
    pub fn build_auth_url(&self, state: &str) -> String {
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&state={}",
            self.authorize_url(),
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(state),
        )
    }

    /// Get the token exchange URL.
    pub fn token_url(&self) -> String {
        format!("{}/auth/token", self.base_url())
    }

    /// Build a JWT client assertion for token exchange (RFC 7523).
    ///
    /// Revolut Business API requires RS256-signed JWT with claims:
    /// - iss: JWT issuer (your app identifier)
    /// - sub: client_id
    /// - aud: token endpoint URL
    /// - iat: issued at timestamp
    /// - exp: expiration (iat + 60 seconds)
    /// - jti: unique token ID (UUID)
    pub fn build_client_assertion(&self) -> Result<String, String> {
        if self.private_key_pem.is_empty() {
            return Err("Private key not configured (REVOLUT_PRIVATE_KEY_PATH)".to_string());
        }
        if self.jwt_issuer.is_empty() {
            return Err("JWT issuer not configured (REVOLUT_JWT_ISSUER)".to_string());
        }

        let iat = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?
            .as_secs()
            .saturating_sub(30); // backdate 30s for clock skew tolerance
        let exp = iat + 90; // 60s validity from real now
        let jti = uuid::Uuid::new_v4().to_string();

        debug!(
            "Revolut JWT assertion: iat={}, exp={}, jti={}",
            iat, exp, jti
        );

        let claims = RevolutJwtClaims {
            iss: self.jwt_issuer.clone(),
            sub: self.client_id.clone(),
            aud: "https://revolut.com".to_string(),
            iat,
            exp,
            jti,
        };

        let header = Header::new(Algorithm::RS256);
        let key = EncodingKey::from_rsa_pem(self.private_key_pem.as_bytes())
            .map_err(|e| format!("Invalid RSA private key: {}", e))?;

        encode(&header, &claims, &key)
            .map_err(|e| format!("JWT encoding failed: {}", e))
    }

    /// Check if JWT client assertion is properly configured.
    pub fn has_jwt_config(&self) -> bool {
        !self.private_key_pem.is_empty() && !self.jwt_issuer.is_empty()
    }
}

/// JWT claims for Revolut client assertion (RFC 7523).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutJwtClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

/// Request body for exchanging authorization code for tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutTokenRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub redirect_uri: Option<String>,
    /// Client assertion JWT (for production, sandbox may not require)
    pub client_assertion_type: Option<String>,
    pub client_assertion: Option<String>,
}

impl RevolutTokenRequest {
    /// Create a token request for authorization code exchange.
    pub fn authorization_code(
        code: String,
        client_id: String,
        redirect_uri: Option<String>,
    ) -> Self {
        Self {
            grant_type: "authorization_code".to_string(),
            code,
            client_id,
            redirect_uri,
            client_assertion_type: None,
            client_assertion: None,
        }
    }

    /// Create a token request for refresh token exchange.
    pub fn refresh_token(refresh_token: String, client_id: String) -> Self {
        Self {
            grant_type: "refresh_token".to_string(),
            code: refresh_token,
            client_id,
            redirect_uri: None,
            client_assertion_type: None,
            client_assertion: None,
        }
    }

    /// Add JWT client assertion to the request (required for production Revolut API).
    pub fn with_assertion(mut self, assertion: String) -> Self {
        self.client_assertion_type =
            Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer".to_string());
        self.client_assertion = Some(assertion);
        self
    }
}

/// Response from token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolutTokenResponse {
    pub access_token: String,
    pub token_type: String,
    /// Token lifetime in seconds.
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl From<RevolutTokenResponse> for NewRevolutAuthToken {
    fn from(resp: RevolutTokenResponse) -> Self {
        NewRevolutAuthToken {
            id: None,
            access_token: resp.access_token,
            refresh_token: resp.refresh_token,
            token_type: Some(resp.token_type),
            scope: resp.scope,
            expires_in: Some(resp.expires_in),
            expires_at: None,
        }
    }
}

// ============================================================================
// API FETCH FUNCTIONS
// ============================================================================

/// Fetch all Revolut Business accounts.
///
/// GET https://b2b.revolut.com/api/1.0/accounts
pub async fn fetch_revolut_accounts(
    token: &str,
    sandbox: bool,
) -> Result<Vec<RevolutApiAccount>, Error> {
    let base_url = if sandbox {
        REVOLUT_SANDBOX_URL
    } else {
        REVOLUT_PRODUCTION_URL
    };

    let url = format!("{}/accounts", base_url);
    debug!("Fetching Revolut accounts from: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| Error::Unexpected(format!("Revolut API request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Unexpected(format!(
            "Revolut API error {}: {}",
            status, body
        )));
    }

    let accounts: Vec<RevolutApiAccount> = response
        .json()
        .await
        .map_err(|e| Error::Unexpected(format!("Failed to parse Revolut accounts: {}", e)))?;

    debug!("Fetched {} Revolut accounts", accounts.len());
    Ok(accounts)
}

/// Fetch transactions for a specific Revolut account.
///
/// GET https://b2b.revolut.com/api/1.0/transactions?account=<account_id>
pub async fn fetch_revolut_transactions(
    token: &str,
    account_id: &str,
    sandbox: bool,
) -> Result<Vec<RevolutApiTransaction>, Error> {
    let base_url = if sandbox {
        REVOLUT_SANDBOX_URL
    } else {
        REVOLUT_PRODUCTION_URL
    };

    let url = format!("{}/transactions?account={}", base_url, account_id);
    debug!("Fetching Revolut transactions from: {}", url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| Error::Unexpected(format!("Revolut API request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Unexpected(format!(
            "Revolut API error {}: {}",
            status, body
        )));
    }

    let transactions: Vec<RevolutApiTransaction> = response
        .json()
        .await
        .map_err(|e| Error::Unexpected(format!("Failed to parse Revolut transactions: {}", e)))?;

    debug!(
        "Fetched {} transactions for account {}",
        transactions.len(),
        account_id
    );
    Ok(transactions)
}

/// Convert Revolut API account to domain model for storage.
pub fn api_account_to_domain(api: RevolutApiAccount) -> RevolutAccount {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    RevolutAccount {
        id: uuid::Uuid::new_v4().to_string(),
        revolut_account_id: api.id,
        name: api.name,
        currency: api.currency,
        balance: api.balance.to_string(),
        state: api.state,
        wf_account_id: None,
        last_synced_at: Some(now.clone()),
        created_at: now.clone(),
        updated_at: now,
    }
}

/// Convert Revolut API transaction to domain model for storage.
pub fn api_transaction_to_domain(api: RevolutApiTransaction, account_id: &str) -> RevolutTransaction {
    // Get the first leg's details (primary transaction)
    let (amount, currency, description, balance_after) = api
        .legs
        .first()
        .map(|leg| {
            (
                leg.amount.to_string(),
                leg.currency.clone(),
                leg.description.clone(),
                leg.balance.map(|b| b.to_string()),
            )
        })
        .unwrap_or((String::new(), String::new(), None, None));

    RevolutTransaction {
        id: uuid::Uuid::new_v4().to_string(),
        revolut_transaction_id: api.id,
        revolut_account_id: account_id.to_string(),
        r#type: api.tx_type,
        state: api.state,
        amount,
        currency,
        description,
        merchant_name: api.merchant.as_ref().and_then(|m| m.name.clone()),
        merchant_category: api.merchant.as_ref().and_then(|m| m.category.clone()),
        reference: api.reference,
        completed_at: api.completed_at,
        created_at: api.created_at,
        balance_after,
        wf_activity_id: None,
        synced_to_activities: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_type_mapping() {
        let tx = RevolutTransaction {
            id: "1".to_string(),
            revolut_transaction_id: "rev-1".to_string(),
            revolut_account_id: "acc-1".to_string(),
            r#type: "TRANSFER".to_string(),
            state: "completed".to_string(),
            amount: "100.00".to_string(),
            currency: "EUR".to_string(),
            description: None,
            merchant_name: None,
            merchant_category: None,
            reference: None,
            completed_at: None,
            created_at: "2026-03-13T00:00:00Z".to_string(),
            balance_after: None,
            wf_activity_id: None,
            synced_to_activities: false,
        };
        
        assert_eq!(tx.map_to_wf_activity_type(), Some(("DEPOSIT", true)));

        let withdraw_tx = RevolutTransaction {
            amount: "-50.00".to_string(),
            ..tx.clone()
        };
        assert_eq!(withdraw_tx.map_to_wf_activity_type(), Some(("WITHDRAWAL", false)));
    }

    #[test]
    fn test_account_state_parsing() {
        assert_eq!(RevolutAccountState::from_str("active"), Some(RevolutAccountState::Active));
        assert_eq!(RevolutAccountState::from_str("blocked"), Some(RevolutAccountState::Blocked));
        assert_eq!(RevolutAccountState::from_str("invalid"), None);
    }

    #[test]
    fn test_build_auth_url() {
        let config = RevolutOAuthConfig {
            client_id: "my-client-id".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            sandbox: true,
            jwt_issuer: String::new(),
            private_key_pem: String::new(),
        };

        let url = config.build_auth_url("random-state-123");
        
        assert!(url.starts_with("https://sandbox-business.revolut.com/app-confirm"));
        assert!(url.contains("client_id=my-client-id"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=random-state-123"));
    }

    #[test]
    fn test_build_auth_url_production() {
        let config = RevolutOAuthConfig {
            client_id: "prod-client".to_string(),
            redirect_uri: "https://app.example.com/oauth".to_string(),
            sandbox: false,
            jwt_issuer: String::new(),
            private_key_pem: String::new(),
        };

        let url = config.build_auth_url("state-456");
        
        assert!(url.starts_with("https://business.revolut.com/app-confirm"));
        assert!(url.contains("client_id=prod-client"));
    }

    #[test]
    fn test_token_response_parsing() {
        let json = r#"{
            "access_token": "oa_sand_abc123",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "oa_sand_refresh_xyz",
            "scope": "READ WRITE"
        }"#;

        let response: RevolutTokenResponse = serde_json::from_str(json).unwrap();
        
        assert_eq!(response.access_token, "oa_sand_abc123");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, 3600);
        assert_eq!(response.refresh_token, Some("oa_sand_refresh_xyz".to_string()));
        assert_eq!(response.scope, Some("READ WRITE".to_string()));
    }

    #[test]
    fn test_token_response_to_new_auth_token() {
        let response = RevolutTokenResponse {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 7200,
            refresh_token: Some("refresh_456".to_string()),
            scope: Some("accounts transactions".to_string()),
        };

        let new_token: NewRevolutAuthToken = response.into();
        
        assert_eq!(new_token.access_token, "access_123");
        assert_eq!(new_token.token_type, Some("Bearer".to_string()));
        assert_eq!(new_token.expires_in, Some(7200));
        assert_eq!(new_token.refresh_token, Some("refresh_456".to_string()));
        assert_eq!(new_token.scope, Some("accounts transactions".to_string()));
        assert!(new_token.id.is_none());
        assert!(new_token.expires_at.is_none());
    }

    #[test]
    fn test_build_client_assertion_missing_config() {
        // Test with missing private key
        let config = RevolutOAuthConfig {
            client_id: "test-client".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            sandbox: false,
            jwt_issuer: "issuer.example.com".to_string(),
            private_key_pem: String::new(),
        };

        assert!(!config.has_jwt_config());
        let result = config.build_client_assertion();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Private key"));

        // Test with missing JWT issuer
        let config2 = RevolutOAuthConfig {
            client_id: "test-client".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            sandbox: false,
            jwt_issuer: String::new(),
            private_key_pem: "some-key".to_string(),
        };

        assert!(!config2.has_jwt_config());
        let result2 = config2.build_client_assertion();
        assert!(result2.is_err());
        assert!(result2.unwrap_err().contains("JWT issuer"));
    }

    #[test]
    fn test_token_request_with_assertion() {
        let request = RevolutTokenRequest::authorization_code(
            "auth-code-123".to_string(),
            "client-id".to_string(),
            Some("https://example.com/callback".to_string()),
        )
        .with_assertion("jwt-token-here".to_string());

        assert_eq!(
            request.client_assertion_type,
            Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer".to_string())
        );
        assert_eq!(request.client_assertion, Some("jwt-token-here".to_string()));
    }

    #[test]
    fn test_fetch_accounts_parsing() {
        // Test parsing of Revolut API accounts response
        let json = r#"[
            {
                "id": "acc-uuid-123",
                "name": "EUR Account",
                "balance": 1234.56,
                "currency": "EUR",
                "state": "active",
                "public": false,
                "created_at": "2024-01-15T10:30:00Z",
                "updated_at": "2026-03-18T14:00:00Z"
            },
            {
                "id": "acc-uuid-456",
                "name": "USD Account",
                "balance": 5000.00,
                "currency": "USD",
                "state": "active",
                "public": true
            }
        ]"#;

        let accounts: Vec<RevolutApiAccount> = serde_json::from_str(json).unwrap();

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].id, "acc-uuid-123");
        assert_eq!(accounts[0].name, "EUR Account");
        assert_eq!(accounts[0].balance, 1234.56);
        assert_eq!(accounts[0].currency, "EUR");
        assert_eq!(accounts[0].state, "active");
        assert!(!accounts[0].public);
        assert_eq!(accounts[1].id, "acc-uuid-456");
        assert!(accounts[1].public);
    }

    #[test]
    fn test_fetch_transactions_parsing() {
        // Test parsing of Revolut API transactions response
        let json = r#"[
            {
                "id": "tx-uuid-789",
                "type": "TRANSFER",
                "state": "completed",
                "created_at": "2026-03-18T12:00:00Z",
                "completed_at": "2026-03-18T12:01:00Z",
                "reference": "Invoice payment",
                "legs": [
                    {
                        "leg_id": "leg-1",
                        "account_id": "acc-uuid-123",
                        "amount": -500.00,
                        "currency": "EUR",
                        "description": "Payment to supplier",
                        "balance": 734.56
                    }
                ],
                "merchant": null
            },
            {
                "id": "tx-uuid-abc",
                "type": "CARD_PAYMENT",
                "state": "completed",
                "created_at": "2026-03-17T09:30:00Z",
                "completed_at": "2026-03-17T09:30:05Z",
                "reference": null,
                "legs": [
                    {
                        "leg_id": "leg-2",
                        "account_id": "acc-uuid-123",
                        "amount": -25.50,
                        "currency": "EUR",
                        "description": "Amazon.com",
                        "balance": 1234.56
                    }
                ],
                "merchant": {
                    "name": "Amazon",
                    "category": "retail"
                }
            }
        ]"#;

        let transactions: Vec<RevolutApiTransaction> = serde_json::from_str(json).unwrap();

        assert_eq!(transactions.len(), 2);

        // First transaction
        assert_eq!(transactions[0].id, "tx-uuid-789");
        assert_eq!(transactions[0].tx_type, "TRANSFER");
        assert_eq!(transactions[0].state, "completed");
        assert_eq!(transactions[0].reference, Some("Invoice payment".to_string()));
        assert_eq!(transactions[0].legs.len(), 1);
        assert_eq!(transactions[0].legs[0].amount, -500.00);
        assert!(transactions[0].merchant.is_none());

        // Second transaction with merchant
        assert_eq!(transactions[1].id, "tx-uuid-abc");
        assert_eq!(transactions[1].tx_type, "CARD_PAYMENT");
        assert!(transactions[1].merchant.is_some());
        let merchant = transactions[1].merchant.as_ref().unwrap();
        assert_eq!(merchant.name, Some("Amazon".to_string()));
        assert_eq!(merchant.category, Some("retail".to_string()));
    }

    #[test]
    fn test_api_account_to_domain() {
        let api = RevolutApiAccount {
            id: "rev-acc-1".to_string(),
            name: "Main EUR".to_string(),
            balance: 999.99,
            currency: "EUR".to_string(),
            state: "active".to_string(),
            public: false,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: None,
        };

        let domain = api_account_to_domain(api);

        assert_eq!(domain.revolut_account_id, "rev-acc-1");
        assert_eq!(domain.name, "Main EUR");
        assert_eq!(domain.balance, "999.99");
        assert_eq!(domain.currency, "EUR");
        assert_eq!(domain.state, "active");
        assert!(domain.wf_account_id.is_none());
        assert!(domain.last_synced_at.is_some());
    }

    #[test]
    fn test_api_transaction_to_domain() {
        let api = RevolutApiTransaction {
            id: "rev-tx-1".to_string(),
            tx_type: "TRANSFER".to_string(),
            state: "completed".to_string(),
            created_at: "2026-03-18T10:00:00Z".to_string(),
            completed_at: Some("2026-03-18T10:00:05Z".to_string()),
            reference: Some("Test ref".to_string()),
            legs: vec![RevolutApiTransactionLeg {
                leg_id: "leg-1".to_string(),
                account_id: "acc-1".to_string(),
                amount: -100.50,
                currency: "EUR".to_string(),
                description: Some("Test payment".to_string()),
                balance: Some(500.00),
            }],
            merchant: Some(RevolutApiMerchant {
                name: Some("Test Shop".to_string()),
                category: Some("retail".to_string()),
            }),
        };

        let domain = api_transaction_to_domain(api, "acc-1");

        assert_eq!(domain.revolut_transaction_id, "rev-tx-1");
        assert_eq!(domain.revolut_account_id, "acc-1");
        assert_eq!(domain.r#type, "TRANSFER");
        assert_eq!(domain.state, "completed");
        assert_eq!(domain.amount, "-100.5");
        assert_eq!(domain.currency, "EUR");
        assert_eq!(domain.description, Some("Test payment".to_string()));
        assert_eq!(domain.merchant_name, Some("Test Shop".to_string()));
        assert_eq!(domain.balance_after, Some("500".to_string()));
        assert!(!domain.synced_to_activities);
    }
}
