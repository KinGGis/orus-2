//! Direct Interactive Brokers integration over IBKR's public MCP server.
//!
//! The flow is a standard OAuth 2.0 authorization code grant with PKCE against
//! a dynamically registered public client. A single human consent produces a
//! refresh token, which is persisted in the secret store and keeps subsequent
//! syncs unattended.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::State,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use rust_decimal::Decimal;
use tracing::{error, info, warn};

use wealthfolio_connect::broker::{SyncConfig, SyncOrchestrator, SyncResult};
use wealthfolio_connect::ibkr::{
    build_authorize_url, generate_pkce, generate_state, http_client, register_client, IbkrMcpClient,
    IbkrTokenManager, IbkrTokenStore, IBKR_ACCOUNT_ID,
};
use wealthfolio_core::errors::{Error as CoreError, Result as CoreResult};
use wealthfolio_core::portfolio::valuation::{
    DailyAccountValuation, ValuationRecalcMode, ValuationSource,
};
use wealthfolio_core::portfolio::snapshot::SnapshotRecalcMode;
use wealthfolio_core::quotes::{DataSource, MarketSyncMode, Quote};
use wealthfolio_core::secrets::SecretStore;

use crate::{
    api::connect::EventBusProgressReporter,
    api::shared::{enqueue_portfolio_job, PortfolioJobConfig},
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
// Allocation override
// ============================================================================

/// IBKR-sourced allocations for an account, or `None` when the local
/// computation should be used instead.
///
/// The local allocation service classifies holdings by joining them against our
/// taxonomy tables, so any unclassified asset lands in "Unknown" and the cards
/// stop reconciling. IBKR already classifies everything it custodies, so for
/// the IBKR account we serve their breakdown directly.
///
/// Returns `None` rather than an error on failure: a broker outage should leave
/// the cards falling back to local data, not blank the page.
pub(crate) async fn allocations_override(
    state: &AppState,
    account_id: &str,
) -> Option<wealthfolio_core::portfolio::allocation::PortfolioAllocations> {
    use wealthfolio_core::accounts::AccountServiceTrait;
    use wealthfolio_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID;

    let refresh_token = state.secret_store.get_secret(IBKR_REFRESH_TOKEN_KEY).ok()??;
    if refresh_token.is_empty() {
        return None;
    }

    let accounts = state.account_service.get_active_non_archived_accounts().ok()?;
    let ibkr = accounts
        .iter()
        .find(|acc| acc.provider_account_id.as_deref() == Some(IBKR_ACCOUNT_ID))?;

    // Serving IBKR's numbers for the whole portfolio is only correct when there
    // is nothing else to aggregate with them.
    let applies = account_id == ibkr.id
        || (account_id == PORTFOLIO_TOTAL_ACCOUNT_ID && accounts.len() == 1);

    if !applies {
        return None;
    }

    let client = match create_ibkr_client(state) {
        Ok(client) => client,
        Err(err) => {
            info!("[IBKR] Not serving allocations, falling back to local: {err}");
            return None;
        }
    };

    match client.allocations().await {
        Ok(allocations) => Some(allocations),
        Err(err) => {
            error!("[IBKR] Allocation fetch failed, falling back to local: {err}");
            None
        }
    }
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

    // IBKR prices its own contracts, which our market data provider cannot
    // resolve from a bare foreign ticker. Importing that history first means
    // the recalculation triggered below values every past day properly.
    if let Err(err) = import_price_history(&state, &client).await {
        warn!("[IBKR] Price history import skipped: {}", err);
    }

    // The MCP publishes IBKR's own daily net asset value. It is the figure the
    // broker itself reports, so it replaces the locally computed market value
    // whenever we have it — that is what repairs the days where a European
    // listing has no quote in our market data. A failure here must not fail the
    // sync: positions and activities are already imported at this point.
    if let Err(err) = import_nav_history(&state, &client).await {
        warn!("[IBKR] NAV history import skipped: {}", err);
    }

    Ok(Json(result))
}

/// Import one year of daily closing prices from IBKR for every instrument the
/// account holds.
///
/// Our market data provider cannot resolve the bare foreign tickers IBKR
/// reports (`AKE`, `RACE`, `SRT`…), so those positions had no price on any past
/// day and were valued at zero throughout the history. IBKR prices its own
/// contracts by `contract_id`, which removes the ambiguity entirely.
///
/// These are genuine provider quotes, not the single-day broker mark, so they
/// are stored under `DataSource::Ibkr` and the valuation service treats them as
/// a real price series. Because quotes are forward-filled when valuing, one bar
/// per trading day also covers weekends and holidays.
///
/// Five years is deliberate rather than generous. Once an asset has any
/// provider quote, the valuation service treats a day without one as a data gap
/// and discards that whole day. A window shorter than the holding period would
/// therefore delete the oldest days from the chart instead of repairing them.
const PRICE_HISTORY_PERIOD: &str = "FIVE_YEARS";

async fn import_price_history(state: &Arc<AppState>, client: &IbkrMcpClient) -> CoreResult<usize> {
    use wealthfolio_connect::broker::BrokerApiClient;

    let holdings = client.get_account_holdings(IBKR_ACCOUNT_ID).await?;
    let positions = holdings.positions.unwrap_or_default();
    if positions.is_empty() {
        return Ok(0);
    }

    let assets_by_symbol: HashMap<String, String> = state
        .asset_service
        .get_assets()?
        .into_iter()
        .filter_map(|asset| {
            let symbol = asset.instrument_symbol.or(asset.display_code)?;
            Some((symbol.to_uppercase(), asset.id))
        })
        .collect();

    let now = Utc::now();
    let mut quotes: Vec<Quote> = Vec::new();

    for position in &positions {
        let Some(symbol) = position
            .symbol
            .as_ref()
            .and_then(|s| s.symbol.as_ref())
            .and_then(|s| s.symbol.clone())
        else {
            continue;
        };

        let Some(contract_id) = position
            .symbol
            .as_ref()
            .and_then(|s| s.id.as_ref())
            .and_then(|id| id.parse::<i64>().ok())
        else {
            warn!("[IBKR] No contract id for {symbol}, skipping its price history");
            continue;
        };

        let Some(asset_id) = assets_by_symbol.get(&symbol.to_uppercase()) else {
            warn!("[IBKR] No local asset matches {symbol}, skipping its price history");
            continue;
        };

        let currency = position
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_default();

        let bars = match client
            .price_history(contract_id, "STK", PRICE_HISTORY_PERIOD)
            .await
        {
            Ok(bars) => bars,
            Err(err) => {
                warn!("[IBKR] Price history unavailable for {symbol}: {err}");
                continue;
            }
        };

        for bar in bars {
            let timestamp = bar
                .date
                .and_hms_opt(12, 0, 0)
                .map(|dt| dt.and_utc())
                .unwrap_or(now);

            quotes.push(Quote {
                id: format!("{}_{}_{}", asset_id, bar.date, DataSource::Ibkr.as_str()),
                asset_id: asset_id.clone(),
                timestamp,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                adjclose: bar.close,
                volume: bar.volume,
                currency: currency.clone(),
                data_source: DataSource::Ibkr,
                created_at: now,
                notes: None,
            });
        }
    }

    if quotes.is_empty() {
        return Ok(0);
    }

    let requested = quotes.len();
    let saved = state.quote_service.bulk_upsert_quotes(quotes).await?;
    info!("[IBKR] Imported {saved}/{requested} historical prices");

    Ok(saved)
}

/// Persist IBKR's reported daily NAV as broker valuations, then ask for a
/// recalculation so the valuation service can fold them into the series.
async fn import_nav_history(state: &Arc<AppState>, client: &IbkrMcpClient) -> CoreResult<usize> {
    use wealthfolio_core::accounts::AccountServiceTrait;

    let accounts = state.account_service.get_all_accounts()?;
    let Some(account) = accounts
        .into_iter()
        .find(|acc| acc.provider_account_id.as_deref() == Some(IBKR_ACCOUNT_ID))
    else {
        return Ok(0);
    };

    let history = client.nav_history().await?;
    if history.points.is_empty() {
        return Ok(0);
    }

    let base_currency = state.base_currency.read().unwrap().clone();
    let currency = history
        .currency
        .clone()
        .unwrap_or_else(|| account.currency.clone());
    let now = Utc::now();

    let valuations: Vec<DailyAccountValuation> = history
        .points
        .iter()
        .map(|(date, nav)| {
            let fx_rate_to_base = if currency == base_currency {
                Decimal::ONE
            } else {
                state
                    .fx_service
                    .get_exchange_rate_for_date(&currency, &base_currency, *date)
                    .unwrap_or(Decimal::ONE)
            };

            DailyAccountValuation {
                id: format!("{}_{}", account.id, date),
                account_id: account.id.clone(),
                valuation_date: *date,
                account_currency: currency.clone(),
                base_currency: base_currency.clone(),
                fx_rate_to_base,
                cash_balance: Decimal::ZERO,
                investment_market_value: *nav,
                total_value: *nav,
                cost_basis: Decimal::ZERO,
                net_contribution: Decimal::ZERO,
                calculated_at: now,
                source: ValuationSource::BrokerImported,
            }
        })
        .collect();

    let imported = valuations.len();
    state.valuation_repository.save_valuations(&valuations).await?;

    info!(
        "[IBKR] Imported {} broker NAV points ({} .. {})",
        imported,
        history.points.first().map(|(d, _)| *d).unwrap(),
        history.points.last().map(|(d, _)| *d).unwrap()
    );

    // Recalculate now that the NAV rows exist: the valuation service merges
    // them into the computed series instead of leaving them as bare NAV.
    enqueue_portfolio_job(
        state.clone(),
        PortfolioJobConfig {
            account_ids: Some(vec![account.id.clone()]),
            market_sync_mode: MarketSyncMode::None,
            snapshot_mode: SnapshotRecalcMode::Full,
            valuation_mode: ValuationRecalcMode::Full,
        },
    );

    Ok(imported)
}

/// Read-only probe that reports what IBKR currently holds, without writing to
/// the database. Useful to compare the broker's truth against stored snapshots.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticResponse {
    positions: usize,
    positions_priced: usize,
    unpriced_symbols: Vec<String>,
    cash_balances: usize,
    activities: usize,
    activities_mapped: usize,
}

async fn diagnostic(State(state): State<Arc<AppState>>) -> ApiResult<Json<DiagnosticResponse>> {
    use wealthfolio_connect::broker::mapping::map_broker_activity;
    use wealthfolio_connect::broker::BrokerApiClient;

    let client = create_ibkr_client(&state)?;

    let holdings = client
        .get_account_holdings(IBKR_ACCOUNT_ID)
        .await
        .map_err(to_api_error)?;
    let activities = client
        .get_account_activities(IBKR_ACCOUNT_ID, None, None, Some(0), Some(100_000))
        .await
        .map_err(to_api_error)?;

    let positions = holdings.positions.unwrap_or_default();
    let unpriced_symbols: Vec<String> = positions
        .iter()
        .filter(|p| p.price.unwrap_or(0.0) <= 0.0)
        .map(|p| {
            p.symbol
                .as_ref()
                .and_then(|s| s.symbol.as_ref())
                .and_then(|s| s.symbol.clone())
                .unwrap_or_else(|| "<unnamed>".to_string())
        })
        .collect();

    let activities_mapped = activities
        .data
        .iter()
        .filter(|a| map_broker_activity(a, IBKR_ACCOUNT_ID, None, None).is_some())
        .count();

    Ok(Json(DiagnosticResponse {
        positions: positions.len(),
        positions_priced: positions.len() - unpriced_symbols.len(),
        unpriced_symbols,
        cash_balances: holdings.balances.map(|b| b.len()).unwrap_or(0),
        activities: activities.data.len(),
        activities_mapped,
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
