//! DFC Revolut Business integration API routes.
//!
//! These routes are DFC-specific for Revolut Business banking integration.

use std::sync::Arc;

use crate::{error::{ApiError, ApiResult}, main_lib::AppState};
use axum::{
    extract::{Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use wealthfolio_core::revolut::{
    api_account_to_domain, api_transaction_to_domain, fetch_revolut_accounts,
    fetch_revolut_transactions, NewRevolutAuthToken, RevolutAccount, RevolutSyncLog,
    RevolutTokenRequest, RevolutTokenResponse, RevolutTransaction,
};

/// GET /api/dfc/revolut/accounts - List all Revolut accounts
async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<RevolutAccount>>> {
    let accounts = state.revolut_repository.list_accounts()?;
    Ok(Json(accounts))
}

/// Response for token status check
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenStatusResponse {
    active: bool,
    expires_at: Option<String>,
}

/// GET /api/dfc/revolut/token/status - Check if there's an active token
async fn get_token_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<TokenStatusResponse>> {
    let token = state.revolut_repository.get_active_token()?;
    
    let response = match token {
        Some(t) => TokenStatusResponse {
            active: true,
            expires_at: Some(t.expires_at),
        },
        None => TokenStatusResponse {
            active: false,
            expires_at: None,
        },
    };
    
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncLogsQuery {
    limit: Option<i64>,
}

/// GET /api/dfc/revolut/sync/logs - Get recent sync logs
async fn get_sync_logs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SyncLogsQuery>,
) -> ApiResult<Json<Vec<RevolutSyncLog>>> {
    let limit = query.limit.unwrap_or(10);
    let logs = state.revolut_repository.get_recent_sync_logs(limit)?;
    Ok(Json(logs))
}

/// GET /api/dfc/revolut/transactions/unsynced - List unsynced transactions
async fn list_unsynced_transactions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<RevolutTransaction>>> {
    let transactions = state.revolut_repository.list_unsynced_transactions()?;
    Ok(Json(transactions))
}

// ============================================================================
// OAuth 2.0 Routes
// ============================================================================

/// Response for GET /dfc/revolut/auth/url
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthUrlResponse {
    url: String,
    state: String,
}

/// GET /api/dfc/revolut/auth/url - Get OAuth authorization URL
async fn get_auth_url(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<AuthUrlResponse>> {
    let config = state
        .revolut_oauth_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Revolut OAuth not configured".to_string()))?;

    // Generate a random state parameter for CSRF protection
    let state_param = uuid::Uuid::new_v4().to_string();
    let url = config.build_auth_url(&state_param);

    Ok(Json(AuthUrlResponse {
        url,
        state: state_param,
    }))
}

/// Request body for POST /dfc/revolut/auth/callback
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthCallbackRequest {
    code: String,
    state: String,
}

/// Response for POST /dfc/revolut/auth/callback
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthCallbackResponse {
    success: bool,
    expires_at: String,
}

/// POST /api/dfc/revolut/auth/callback - Exchange authorization code for tokens
async fn auth_callback(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AuthCallbackRequest>,
) -> ApiResult<Json<AuthCallbackResponse>> {
    let config = state
        .revolut_oauth_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Revolut OAuth not configured".to_string()))?;

    // Build token request
    let mut token_request = RevolutTokenRequest::authorization_code(
        payload.code,
        config.client_id.clone(),
        Some(config.redirect_uri.clone()),
    );

    // Add JWT client assertion if configured (required for production)
    if config.has_jwt_config() {
        let assertion = config
            .build_client_assertion()
            .map_err(|e| ApiError::Internal(format!("Failed to build client assertion: {}", e)))?;
        token_request = token_request.with_assertion(assertion);
    }

    // Exchange code for token via HTTP
    let client = reqwest::Client::new();
    let response = client
        .post(config.token_url())
        .form(&token_request)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Token exchange failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::Internal(format!(
            "Token exchange failed with status {}: {}",
            status, body
        )));
    }

    let token_response: RevolutTokenResponse = response
        .json()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to parse token response: {}", e)))?;

    // Convert to NewRevolutAuthToken and save
    let new_token: NewRevolutAuthToken = token_response.into();
    let saved_token = state.revolut_repository.save_token(new_token)?;

    Ok(Json(AuthCallbackResponse {
        success: true,
        expires_at: saved_token.expires_at,
    }))
}

/// DELETE /api/dfc/revolut/auth/token - Delete all authentication tokens
async fn delete_auth_token(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    state.revolut_repository.delete_all_tokens()?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// Sync Routes
// ============================================================================

/// Response for POST /dfc/revolut/sync
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResponse {
    accounts_synced: i32,
    transactions_synced: i32,
    sync_log_id: String,
}

/// POST /api/dfc/revolut/sync - Sync accounts and transactions from Revolut
async fn sync_revolut(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SyncResponse>> {
    // 1. Get active token
    let token = state
        .revolut_repository
        .get_active_token()?
        .ok_or_else(|| ApiError::Unauthorized("No active Revolut token".to_string()))?;

    // Check if token is expired
    let expires_at = chrono::DateTime::parse_from_rfc3339(&token.expires_at)
        .map_err(|_| ApiError::Internal("Invalid token expiration date".to_string()))?;
    if expires_at < chrono::Utc::now() {
        return Err(ApiError::Unauthorized("Revolut token has expired".to_string()));
    }

    // 2. Get OAuth config for sandbox flag
    let config = state
        .revolut_oauth_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("Revolut OAuth not configured".to_string()))?;

    // 3. Create sync log
    let sync_log = state.revolut_repository.create_sync_log("full")?;

    // 4. Fetch and upsert accounts
    let api_accounts = match fetch_revolut_accounts(&token.access_token, config.sandbox).await {
        Ok(accounts) => accounts,
        Err(e) => {
            let _ = state.revolut_repository.fail_sync_log(&sync_log.id, &e.to_string());
            return Err(ApiError::Internal(format!("Failed to fetch accounts: {}", e)));
        }
    };

    let mut accounts_synced = 0;
    for api_account in &api_accounts {
        let account = api_account_to_domain(api_account.clone());
        if let Err(e) = state.revolut_repository.upsert_account(account) {
            tracing::warn!("Failed to upsert account {}: {}", api_account.id, e);
        } else {
            accounts_synced += 1;
        }
    }

    // 5. Fetch and upsert transactions for each account
    let mut transactions_synced = 0;
    for api_account in &api_accounts {
        match fetch_revolut_transactions(&token.access_token, &api_account.id, config.sandbox).await
        {
            Ok(api_transactions) => {
                for api_tx in api_transactions {
                    let tx = api_transaction_to_domain(api_tx, &api_account.id);
                    if let Err(e) = state.revolut_repository.upsert_transaction(tx) {
                        tracing::warn!("Failed to upsert transaction: {}", e);
                    } else {
                        transactions_synced += 1;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch transactions for account {}: {}",
                    api_account.id,
                    e
                );
            }
        }
    }

    // 6. Complete sync log
    state
        .revolut_repository
        .complete_sync_log(&sync_log.id, accounts_synced, transactions_synced)?;

    Ok(Json(SyncResponse {
        accounts_synced,
        transactions_synced,
        sync_log_id: sync_log.id,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dfc/revolut/accounts", get(list_accounts))
        .route("/dfc/revolut/token/status", get(get_token_status))
        .route("/dfc/revolut/sync/logs", get(get_sync_logs))
        .route(
            "/dfc/revolut/transactions/unsynced",
            get(list_unsynced_transactions),
        )
        // OAuth 2.0 routes
        .route("/dfc/revolut/auth/url", get(get_auth_url))
        .route("/dfc/revolut/auth/callback", post(auth_callback))
        .route("/dfc/revolut/auth/token", delete(delete_auth_token))
        // Sync routes
        .route("/dfc/revolut/sync", post(sync_revolut))
}
