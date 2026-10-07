//! Direct Interactive Brokers integration over IBKR's public MCP server.
//!
//! The flow is a standard OAuth 2.0 authorization code grant with PKCE against
//! a dynamically registered public client. A single human consent produces a
//! refresh token, which is persisted in the secret store and keeps subsequent
//! syncs unattended.

use std::sync::Arc;

use axum::{
    extract::State,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use wealthfolio_connect::broker::{SyncConfig, SyncOrchestrator, SyncResult};
use wealthfolio_connect::ibkr::{
    build_authorize_url, generate_pkce, generate_state, http_client, register_client, IbkrMcpClient,
    IbkrTokenManager, IbkrTokenStore, IBKR_ACCOUNT_ID,
};
use wealthfolio_core::errors::{Error as CoreError, Result as CoreResult};
use wealthfolio_core::secrets::SecretStore;

use crate::{
    api::connect::EventBusProgressReporter,
    error::{ApiError, ApiResult},
    main_lib::AppState,
};

/// OAuth client id obtained from IBKR's dynamic client registration.
const IBKR_CLIENT_ID_KEY: &str = "ibkr_client_id";
/// Long-lived, rotating refresh token.
const IBKR_REFRESH_TOKEN_KEY: &str = "ibkr_refresh_token";
/// PKCE verifier, held only between the redirect and the token exchange.
const IBKR_PKCE_VERIFIER_KEY: &str = "ibkr_pkce_verifier";
/// CSRF `state` value for the in-flight authorization.
const IBKR_OAUTH_STATE_KEY: &str = "ibkr_oauth_state";
/// Redirect URI of the in-flight authorization; it must match at exchange time.
const IBKR_REDIRECT_URI_KEY: &str = "ibkr_redirect_uri";

/// Adapts the application secret store to the token store the IBKR client needs.
struct SecretStoreTokens {
    secrets: Arc<dyn SecretStore>,
}

impl IbkrTokenStore for SecretStoreTokens {
    fn load_refresh_token(&self) -> CoreResult<Option<String>> {
        self.secrets.get_secret(IBKR_REFRESH_TOKEN_KEY)
    }

    fn save_refresh_token(&self, token: &str) -> CoreResult<()> {
        self.secrets.set_secret(IBKR_REFRESH_TOKEN_KEY, token)
    }

    fn clear_refresh_token(&self) -> CoreResult<()> {
        self.secrets.delete_secret(IBKR_REFRESH_TOKEN_KEY)
    }
}

/// Build an MCP-backed broker client from the stored credentials.
fn create_ibkr_client(state: &AppState) -> ApiResult<IbkrMcpClient> {
    let client_id = state
        .secret_store
        .get_secret(IBKR_CLIENT_ID_KEY)?
        .ok_or_else(|| {
            ApiError::Unauthorized("IBKR is not connected: no OAuth client registered".to_string())
        })?;

    let tokens = Arc::new(IbkrTokenManager::new(
        http_client(),
        client_id,
        Arc::new(SecretStoreTokens {
            secrets: state.secret_store.clone(),
        }),
    ));

    if !tokens.is_connected() {
        return Err(ApiError::Unauthorized(
            "IBKR is not connected: authorize the integration first".to_string(),
        ));
    }

    Ok(IbkrMcpClient::new(http_client(), tokens))
}

// ============================================================================
// Routes
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    connected: bool,
    client_registered: bool,
}

async fn get_status(State(state): State<Arc<AppState>>) -> ApiResult<Json<StatusResponse>> {
    let client_registered = state
        .secret_store
        .get_secret(IBKR_CLIENT_ID_KEY)?
        .is_some();
    let connected = state
        .secret_store
        .get_secret(IBKR_REFRESH_TOKEN_KEY)?
        .is_some();

    Ok(Json(StatusResponse {
        connected,
        client_registered,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthUrlRequest {
    /// Must exactly match the redirect URI used at token exchange time.
    redirect_uri: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthUrlResponse {
    auth_url: String,
    state: String,
}

/// Start an authorization: register the OAuth client on first use, mint a PKCE
/// pair, and hand back the consent URL.
async fn get_auth_url(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AuthUrlRequest>,
) -> ApiResult<Json<AuthUrlResponse>> {
    if body.redirect_uri.trim().is_empty() {
        return Err(ApiError::BadRequest("redirectUri is required".to_string()));
    }

    let client_id = match state.secret_store.get_secret(IBKR_CLIENT_ID_KEY)? {
        Some(existing) => existing,
        None => {
            let registered = register_client(
                &http_client(),
                "Orus Portfolio Sync",
                &body.redirect_uri,
            )
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
            state
                .secret_store
                .set_secret(IBKR_CLIENT_ID_KEY, &registered)?;
            info!("[IBKR] Registered a new OAuth client with IBKR");
            registered
        }
    };

    let pkce = generate_pkce();
    let csrf_state = generate_state();

    state
        .secret_store
        .set_secret(IBKR_PKCE_VERIFIER_KEY, &pkce.verifier)?;
    state
        .secret_store
        .set_secret(IBKR_OAUTH_STATE_KEY, &csrf_state)?;
    state
        .secret_store
        .set_secret(IBKR_REDIRECT_URI_KEY, &body.redirect_uri)?;

    Ok(Json(AuthUrlResponse {
        auth_url: build_authorize_url(
            &client_id,
            &body.redirect_uri,
            &pkce.challenge,
            &csrf_state,
        ),
        state: csrf_state,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallbackRequest {
    code: String,
    state: String,
}

/// Complete the authorization by exchanging the code for tokens.
async fn handle_callback(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CallbackRequest>,
) -> ApiResult<Json<StatusResponse>> {
    let expected_state = state
        .secret_store
        .get_secret(IBKR_OAUTH_STATE_KEY)?
        .ok_or_else(|| {
            ApiError::BadRequest("No IBKR authorization is in progress".to_string())
        })?;
    if expected_state != body.state {
        return Err(ApiError::BadRequest(
            "IBKR authorization state mismatch".to_string(),
        ));
    }

    let verifier = state
        .secret_store
        .get_secret(IBKR_PKCE_VERIFIER_KEY)?
        .ok_or_else(|| ApiError::BadRequest("Missing IBKR PKCE verifier".to_string()))?;
    let redirect_uri = state
        .secret_store
        .get_secret(IBKR_REDIRECT_URI_KEY)?
        .ok_or_else(|| ApiError::BadRequest("Missing IBKR redirect URI".to_string()))?;
    let client_id = state
        .secret_store
        .get_secret(IBKR_CLIENT_ID_KEY)?
        .ok_or_else(|| ApiError::BadRequest("No IBKR OAuth client registered".to_string()))?;

    let tokens = IbkrTokenManager::new(
        http_client(),
        client_id,
        Arc::new(SecretStoreTokens {
            secrets: state.secret_store.clone(),
        }),
    );
    tokens
        .exchange_code(&body.code, &redirect_uri, &verifier)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // The single-use authorization artefacts must not outlive the exchange.
    let _ = state.secret_store.delete_secret(IBKR_PKCE_VERIFIER_KEY);
    let _ = state.secret_store.delete_secret(IBKR_OAUTH_STATE_KEY);

    info!("[IBKR] Authorization complete; refresh token stored");
    Ok(Json(StatusResponse {
        connected: true,
        client_registered: true,
    }))
}

/// Forget the IBKR grant. The OAuth client registration is kept so a
/// reconnection does not need to register again.
async fn disconnect(State(state): State<Arc<AppState>>) -> ApiResult<Json<StatusResponse>> {
    state.secret_store.delete_secret(IBKR_REFRESH_TOKEN_KEY)?;
    let _ = state.secret_store.delete_secret(IBKR_PKCE_VERIFIER_KEY);
    let _ = state.secret_store.delete_secret(IBKR_OAUTH_STATE_KEY);
    let _ = state.secret_store.delete_secret(IBKR_REDIRECT_URI_KEY);

    Ok(Json(StatusResponse {
        connected: false,
        client_registered: state
            .secret_store
            .get_secret(IBKR_CLIENT_ID_KEY)?
            .is_some(),
    }))
}

/// The MCP reports broker-side positions and balances exactly, but its trade
/// feed only covers five quarters and excludes dividends, fees and interest.
/// Rebuilding a NAV from those trades would therefore drift, so the IBKR
/// account must track HOLDINGS. That mode still imports the trades for
/// reference, so nothing is lost. A freshly created account starts as NOT_SET
/// and the orchestrator skips it, so we flip it before syncing data.
async fn ensure_holdings_tracking_mode(state: &AppState) -> ApiResult<bool> {
    use wealthfolio_core::accounts::{AccountServiceTrait, AccountUpdate, TrackingMode};

    let accounts = state
        .account_service
        .get_all_accounts()
        .map_err(to_api_error)?;

    let Some(account) = accounts.into_iter().find(|acc| {
        acc.provider_account_id.as_deref() == Some(IBKR_ACCOUNT_ID)
    }) else {
        return Ok(false);
    };

    if account.tracking_mode == TrackingMode::Holdings {
        return Ok(false);
    }

    info!(
        "[IBKR] Switching account '{}' to HOLDINGS tracking (was {:?})",
        account.name, account.tracking_mode
    );

    state
        .account_service
        .update_account(AccountUpdate {
            id: Some(account.id.clone()),
            name: account.name.clone(),
            account_type: account.account_type.clone(),
            group: account.group.clone(),
            is_default: account.is_default,
            is_active: account.is_active,
            platform_id: account.platform_id.clone(),
            account_number: account.account_number.clone(),
            meta: account.meta.clone(),
            provider: account.provider.clone(),
            provider_account_id: account.provider_account_id.clone(),
            is_archived: Some(account.is_archived),
            tracking_mode: Some(TrackingMode::Holdings),
        })
        .await
        .map_err(to_api_error)?;

    Ok(true)
}

/// Run a full sync through the shared orchestrator, so IBKR benefits from the
/// same pagination, resume and snapshot handling as every other broker.
async fn sync_ibkr(State(state): State<Arc<AppState>>) -> ApiResult<Json<SyncResult>> {
    let client = create_ibkr_client(&state)?;

    let reporter = Arc::new(EventBusProgressReporter::new(state.event_bus.clone()));
    let orchestrator = SyncOrchestrator::new(
        state.connect_sync_service.clone(),
        reporter,
        SyncConfig::default(),
    );

    ensure_holdings_tracking_mode(&state).await?;

    let mut result = match orchestrator.sync_all(&client).await {
        Ok(result) => result,
        Err(err) => {
            error!("[IBKR] Sync failed: {}", err);
            return Err(ApiError::Internal(err));
        }
    };

    // The very first sync creates the account as NOT_SET, so the orchestrator
    // skipped its data. Flip it and replay immediately to keep this one click.
    if ensure_holdings_tracking_mode(&state).await? {
        result = orchestrator
            .sync_all(&client)
            .await
            .map_err(|err| {
                error!("[IBKR] Sync failed after enabling holdings tracking: {}", err);
                ApiError::Internal(err)
            })?;
    }

    info!("[IBKR] Sync completed: {}", result.message);
    Ok(Json(result))
}

/// Read-only probe that reports what IBKR currently holds, without writing to
/// the database. Useful to compare the broker's truth against stored snapshots.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticResponse {
    positions: usize,
    cash_balances: usize,
    activities: usize,
}

async fn diagnostic(State(state): State<Arc<AppState>>) -> ApiResult<Json<DiagnosticResponse>> {
    use wealthfolio_connect::broker::BrokerApiClient;

    let client = create_ibkr_client(&state)?;

    let holdings = client
        .get_account_holdings(IBKR_ACCOUNT_ID)
        .await
        .map_err(to_api_error)?;
    let activities = client
        .get_account_activities(IBKR_ACCOUNT_ID, None, None, Some(0), Some(1))
        .await
        .map_err(to_api_error)?;

    Ok(Json(DiagnosticResponse {
        positions: holdings.positions.map(|p| p.len()).unwrap_or(0),
        cash_balances: holdings.balances.map(|b| b.len()).unwrap_or(0),
        activities: activities
            .pagination
            .and_then(|p| p.total)
            .unwrap_or(0)
            .max(0) as usize,
    }))
}

fn to_api_error(err: CoreError) -> ApiError {
    ApiError::Internal(err.to_string())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dfc/ibkr/status", get(get_status))
        .route("/dfc/ibkr/auth-url", post(get_auth_url))
        .route("/dfc/ibkr/callback", post(handle_callback))
        .route("/dfc/ibkr/connection", delete(disconnect))
        .route("/dfc/ibkr/sync", post(sync_ibkr))
        .route("/dfc/ibkr/diagnostic", get(diagnostic))
}
