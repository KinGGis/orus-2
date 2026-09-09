//! DFC SnapTrade integration API routes.
//!
//! These routes provide SnapTrade brokerage connection capabilities.
//! User secrets (userId + userSecret) are stored in secret_store, not the database.

use std::collections::HashMap;
use std::sync::Arc;
use reqwest;

use crate::{
    api::shared::{enqueue_portfolio_job, PortfolioJobConfig},
    error::{ApiError, ApiResult},
    main_lib::AppState,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use wealthfolio_core::accounts::{AccountServiceTrait, NewAccount, TrackingMode};
use wealthfolio_core::activities::{ActivityStatus, ActivityUpsert};
use wealthfolio_core::assets::AssetMetadata;
use wealthfolio_core::portfolio::snapshot::{
    AccountStateSnapshot, Position, SnapshotRecalcMode, SnapshotSource,
};
use wealthfolio_core::portfolio::valuation::ValuationRecalcMode;
use wealthfolio_core::quotes::MarketSyncMode;
use wealthfolio_core::snaptrade::{self, SnapTradeAccount};
use std::collections::HashSet;

// Secret store keys for SnapTrade credentials
const SNAPTRADE_USER_ID_KEY: &str = "snaptrade_user_id";
const SNAPTRADE_USER_SECRET_KEY: &str = "snaptrade_user_secret";

/// Identifies the running build so a diagnostic response can prove which commit
/// is actually deployed. Render injects `RENDER_GIT_COMMIT`; fall back to the
/// commit baked in at compile time, then to a placeholder.
fn build_marker() -> String {
    std::env::var("RENDER_GIT_COMMIT")
        .ok()
        .or_else(|| option_env!("GIT_COMMIT_SHA").map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

// ============================================================================
// Response DTOs (transformed for frontend)
// ============================================================================

/// Connection response for frontend (flattened from SnapTradeConnection)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionResponse {
    id: String,
    name: Option<String>,
    institution_name: Option<String>,
    created_date: Option<String>,
}

// ============================================================================
// Status Route
// ============================================================================

/// Response for GET /dfc/snaptrade/status
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapTradeStatusResponse {
    configured: bool,
    registered: bool,
}

/// GET /api/dfc/snaptrade/status - Check SnapTrade configuration and registration status
async fn get_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SnapTradeStatusResponse>> {
    let configured = state.snaptrade_config.is_some();
    
    // Check if user is registered by looking for stored credentials
    let registered = if configured {
        let user_id = state.secret_store.get_secret(SNAPTRADE_USER_ID_KEY).ok().flatten();
        let user_secret = state.secret_store.get_secret(SNAPTRADE_USER_SECRET_KEY).ok().flatten();
        user_id.is_some() && user_secret.is_some()
    } else {
        false
    };

    Ok(Json(SnapTradeStatusResponse { configured, registered }))
}

// ============================================================================
// User Registration Routes
// ============================================================================

/// Request for POST /dfc/snaptrade/register
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequest {
    /// User identifier to register with SnapTrade (typically the app's internal user ID)
    user_id: String,
}

/// Response for POST /dfc/snaptrade/register
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterResponse {
    success: bool,
    user_id: String,
}

/// POST /api/dfc/snaptrade/register - Register a new SnapTrade user
async fn register_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> ApiResult<Json<RegisterResponse>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    // Check if already registered
    let existing_user_id = state.secret_store.get_secret(SNAPTRADE_USER_ID_KEY).ok().flatten();
    if existing_user_id.is_some() {
        return Err(ApiError::BadRequest("SnapTrade user already registered. Delete user first.".to_string()));
    }

    // Register with SnapTrade API. When no local secret exists but the SnapTrade
    // user was previously registered (e.g. the ephemeral secret store was wiped on a
    // redeploy), the initial registration fails because the userId already exists on
    // SnapTrade's side. In that case, delete the stale remote user and retry once so
    // the account can recover a fresh, persistent user secret. This is safe here: we
    // only reach this point after confirming no working local credentials exist, so
    // there is no functional connection to preserve.
    let user_secret = match snaptrade::register_user(config, &payload.user_id).await {
        Ok(secret) => secret,
        Err(first_err) => {
            tracing::warn!(
                "SnapTrade registration failed ({}); deleting any stale remote user '{}' and retrying",
                first_err,
                payload.user_id
            );
            if let Err(e) = snaptrade::delete_snaptrade_user(config, &payload.user_id).await {
                tracing::warn!("Failed to delete stale SnapTrade user before retry: {}", e);
            }
            snaptrade::register_user(config, &payload.user_id)
                .await
                .map_err(|e| ApiError::Internal(format!("SnapTrade registration failed: {}", e)))?
        }
    };

    // Store credentials in secret_store
    state.secret_store.set_secret(SNAPTRADE_USER_ID_KEY, &user_secret.user_id)?;
    state.secret_store.set_secret(SNAPTRADE_USER_SECRET_KEY, &user_secret.user_secret)?;

    Ok(Json(RegisterResponse {
        success: true,
        user_id: user_secret.user_id,
    }))
}

/// DELETE /api/dfc/snaptrade/user - Delete SnapTrade user and clear stored credentials
async fn delete_user(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    // Get stored user_id
    let user_id = state
        .secret_store
        .get_secret(SNAPTRADE_USER_ID_KEY)?
        .ok_or_else(|| ApiError::BadRequest("No SnapTrade user registered".to_string()))?;

    // Delete from SnapTrade API (best effort - continue even if this fails)
    if let Err(e) = snaptrade::delete_snaptrade_user(config, &user_id).await {
        tracing::warn!("Failed to delete SnapTrade user remotely: {}", e);
    }

    // Clear local credentials regardless
    state.secret_store.delete_secret(SNAPTRADE_USER_ID_KEY)?;
    state.secret_store.delete_secret(SNAPTRADE_USER_SECRET_KEY)?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// Connection Routes
// ============================================================================

/// Request for POST /dfc/snaptrade/connect-url
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectUrlRequest {
    redirect_uri: String,
}

/// Response for POST /dfc/snaptrade/connect-url
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectUrlResponse {
    url: String,
}

/// POST /api/dfc/snaptrade/connect-url - Get SnapTrade connection portal URL
async fn get_connect_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConnectUrlRequest>,
) -> ApiResult<Json<ConnectUrlResponse>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    let url = snaptrade::get_login_url(config, &user_id, &user_secret, &payload.redirect_uri)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to get SnapTrade login URL: {}", e)))?;

    Ok(Json(ConnectUrlResponse { url }))
}

/// GET /api/dfc/snaptrade/connections - List all brokerage connections
async fn list_connections(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<ConnectionResponse>>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    let connections = snaptrade::list_connections(config, &user_id, &user_secret)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list connections: {}", e)))?;

    // Transform to frontend-expected format
    let response: Vec<ConnectionResponse> = connections
        .into_iter()
        .map(|c| {
            let institution_name = c.institution_name().map(|s| s.to_string());
            ConnectionResponse {
                id: c.id,
                name: c.name,
                institution_name,
                created_date: c.created_date,
            }
        })
        .collect();

    Ok(Json(response))
}

/// DELETE /api/dfc/snaptrade/connections/:auth_id - Delete a specific brokerage connection
async fn delete_connection(
    State(state): State<Arc<AppState>>,
    Path(auth_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    snaptrade::delete_connection(config, &user_id, &user_secret, &auth_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to delete connection: {}", e)))?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============================================================================
// Account Routes
// ============================================================================

/// GET /api/dfc/snaptrade/accounts - List all accounts across all connections
async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<SnapTradeAccount>>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    let accounts = snaptrade::list_accounts(config, &user_id, &user_secret)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list accounts: {}", e)))?;

    Ok(Json(accounts))
}

// ============================================================================
// Sync Routes
// ============================================================================

/// Response for POST /dfc/snaptrade/sync
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapTradeSyncResponse {
    accounts_synced: usize,
    activities_synced: usize,
    raw_activities_from_snaptrade: usize,
    skipped_unmapped_account: usize,
    saved: usize,
    holdings_synced: usize,
    positions_count: usize,
    cash_balances_count: usize,
}

/// Account info for diagnostic response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticAccountInfo {
    id: String,
    name: Option<String>,
    sync_status: Option<String>,
    number: Option<String>,
}

/// Response for GET /dfc/snaptrade/sync-diagnostic
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncDiagnosticResponse {
    /// Hardcoded marker to confirm which backend build is actually live. Bump this
    /// whenever the diagnostic changes so a stale deploy is immediately obvious.
    build_marker: String,
    snaptrade_accounts: Vec<DiagnosticAccountInfo>,
    raw_activities_count: usize,
    activity_account_ids_seen: Vec<String>,
    account_map: HashMap<String, String>,
    mismatched_ids: Vec<String>,
    date_range: DateRange,
    account_diagnostics: Vec<AccountDiagnostic>,
}

/// Per-account ground-truth diagnostic (DB state, not SnapTrade API state).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountDiagnostic {
    wf_account_id: String,
    activity_count: usize,
    activity_type_counts: HashMap<String, usize>,
    activities_with_asset_id: usize,
    activities_without_asset_id: usize,
    buy_sell_count: usize,
    latest_snapshot_date: Option<String>,
    latest_snapshot_source: Option<String>,
    latest_snapshot_positions_count: usize,
    /// Total number of position rows in the latest snapshot, INCLUDING zero-quantity
    /// positions. If this is > 0 while latest_snapshot_positions_count (non-zero) is
    /// 0, positions are being created but never accumulate quantity (e.g. the BUY lot
    /// was dropped because a currency conversion failed), so every holding is filtered
    /// out of the view.
    latest_snapshot_raw_positions_count: usize,
    /// Sample of up to 8 positions from the latest snapshot as "asset_id=qty@ccy",
    /// including zero-quantity ones, to reveal whether quantities are actually zero.
    latest_snapshot_position_quantities_sample: Vec<String>,
    latest_snapshot_position_asset_ids_sample: Vec<String>,
    /// How many of the latest snapshot's position asset_ids actually resolve to an
    /// existing asset row (via the same lookup the holdings view uses). If this is
    /// less than latest_snapshot_positions_count, positions are being dropped at
    /// display time because their asset does not exist.
    latest_snapshot_position_assets_resolved: usize,
    latest_snapshot_cash_currencies: Vec<String>,
    /// Ground-truth inventory of ALL snapshots stored for this account (any source).
    /// If snapshot_total_count has no "Calculated" entry in snapshot_source_counts,
    /// the recalc engine is producing/persisting nothing at all for this account.
    snapshot_total_count: usize,
    /// Count of stored snapshots grouped by source (e.g. {"BrokerImported":1,"Calculated":420}).
    snapshot_source_counts: HashMap<String, usize>,
    /// Earliest / latest snapshot dates across ALL sources.
    snapshot_date_min: Option<String>,
    snapshot_date_max: Option<String>,
    /// The latest CALCULATED frame specifically (independent of any broker anchor).
    /// If this is present with a recent date and non-zero positions but the holdings
    /// view is still empty, the problem is downstream (query/display), not recalc.
    latest_calculated_snapshot_date: Option<String>,
    latest_calculated_raw_positions_count: usize,
    latest_calculated_nonzero_positions_count: usize,
    latest_calculated_quantities_sample: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DateRange {
    start: String,
    end: String,
}

/// POST /api/dfc/snaptrade/sync - Sync accounts and activities from SnapTrade to WF database
async fn sync_snaptrade(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SnapTradeSyncResponse>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    // 1. Fetch SnapTrade accounts
    let st_accounts = snaptrade::list_accounts(config, &user_id, &user_secret)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list accounts: {}", e)))?;

    // 2. Fetch existing WF accounts
    let wf_accounts = state
        .account_service
        .get_active_accounts()
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 3. Upsert WF accounts and build mapping
    let mut account_map: HashMap<String, String> = HashMap::new(); // snaptrade_id -> wf_id

    for st_account in &st_accounts {
        // Check if WF account already exists
        let existing = wf_accounts.iter().find(|a| {
            a.provider.as_deref() == Some("SNAPTRADE")
                && a.provider_account_id.as_deref() == Some(&st_account.id)
        });

        let wf_account_id = if let Some(existing_account) = existing {
            existing_account.id.clone()
        } else {
            // Create new account
            let new_account = NewAccount {
                id: None,
                name: format!(
                    "{} - {}",
                    st_account.institution_name.as_deref().unwrap_or("SnapTrade"),
                    st_account.name.as_deref().unwrap_or(&st_account.id)
                ),
                account_type: "BROKERAGE".to_string(),
                group: None,
                currency: st_account
                    .currency
                    .as_ref()
                    .and_then(|c| c.code.clone())
                    .unwrap_or_else(|| "USD".to_string()),
                is_default: false,
                is_active: true,
                platform_id: None,
                account_number: st_account.number.clone(),
                meta: None,
                provider: Some("SNAPTRADE".to_string()),
                provider_account_id: Some(st_account.id.clone()),
                is_archived: false,
                tracking_mode: TrackingMode::Transactions,
            };

            let created = state
                .account_service
                .create_account(new_account)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            created.id
        };

        account_map.insert(st_account.id.clone(), wf_account_id);
    }

    // 4. Compute date range (last 2 years)
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let start = (chrono::Utc::now() - chrono::Duration::days(730))
        .format("%Y-%m-%d")
        .to_string();

    // 5. Fetch SnapTrade activities
    let st_activities = snaptrade::list_activities(config, &user_id, &user_secret, Some(&start), Some(&today))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list activities: {}", e)))?;

    let raw_activities_count = st_activities.len();

    // 6. Build ActivityUpsert list
    let mut activity_upserts: Vec<ActivityUpsert> = Vec::new();
    let mut skipped_count: usize = 0;
    // Cache resolved asset UUIDs by symbol. Activities reference assets by their
    // internal id (a UUID in the Postgres backend), not by their ticker, so each
    // symbol must be resolved to (or created as) an asset before upsert.
    let mut activity_asset_id_cache: HashMap<String, String> = HashMap::new();

    for st_act in &st_activities {
        // Get WF account ID from mapping
        let st_account_id = st_act
            .account
            .as_ref()
            .and_then(|a| a.id.clone());

        let wf_account_id = match st_account_id {
            Some(ref id) => match account_map.get(id) {
                Some(wf_id) => wf_id.clone(),
                None => {
                    tracing::warn!(
                        "SnapTrade activity {} skipped: account_id {:?} not found in account_map. \
                         Known SnapTrade account IDs: {:?}",
                        st_act.id.as_deref().unwrap_or("unknown"),
                        id,
                        account_map.keys().collect::<Vec<_>>()
                    );
                    skipped_count += 1;
                    continue;
                }
            },
            None => {
                tracing::warn!(
                    "SnapTrade activity {} skipped: no account_id present in activity",
                    st_act.id.as_deref().unwrap_or("unknown")
                );
                skipped_count += 1;
                continue;
            }
        };

        let activity_id = st_act
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Resolve the ticker to an internal asset id (UUID). Symbol-less activities
        // (deposits, withdrawals, fees, ...) legitimately have no asset.
        let asset_id = match st_act
            .symbol
            .as_ref()
            .and_then(|s| s.symbol.clone())
            .filter(|s| !s.trim().is_empty())
        {
            Some(symbol) => {
                if let Some(cached) = activity_asset_id_cache.get(&symbol) {
                    Some(cached.clone())
                } else {
                    let currency = st_act.currency.as_ref().and_then(|c| c.code.clone());
                    let metadata = AssetMetadata {
                        instrument_symbol: Some(symbol.clone()),
                        display_code: Some(symbol.clone()),
                        requested_quote_ccy: currency.clone(),
                        ..Default::default()
                    };
                    match state
                        .asset_service
                        .get_or_create_minimal_asset(&symbol, currency, Some(metadata), None)
                        .await
                    {
                        Ok(asset) => {
                            activity_asset_id_cache.insert(symbol.clone(), asset.id.clone());
                            Some(asset.id)
                        }
                        Err(e) => {
                            tracing::warn!(
                                "SnapTrade activity {} skipped: failed to resolve asset for symbol {}: {}",
                                st_act.id.as_deref().unwrap_or("unknown"),
                                symbol,
                                e
                            );
                            skipped_count += 1;
                            continue;
                        }
                    }
                }
            }
            None => None,
        };

        let activity_upsert = ActivityUpsert {
            id: activity_id.clone(),
            account_id: wf_account_id,
            asset_id,
            activity_type: snaptrade::map_snaptrade_action(
                st_act.action.as_deref().unwrap_or("OTHER"),
            )
            .to_string(),
            subtype: None,
            activity_date: st_act
                .trade_date
                .clone()
                .or_else(|| st_act.settlement_date.clone())
                .unwrap_or_else(|| today.clone()),
            quantity: st_act.units.and_then(|u| Decimal::try_from(u).ok()),
            unit_price: st_act.price.and_then(|p| Decimal::try_from(p).ok()),
            currency: st_act
                .currency
                .as_ref()
                .and_then(|c| c.code.clone())
                .unwrap_or_else(|| "USD".to_string()),
            fee: None,
            amount: st_act.amount.and_then(|a| Decimal::try_from(a).ok()),
            status: Some(ActivityStatus::Posted),
            notes: st_act.description.clone(),
            fx_rate: None,
            metadata: None,
            needs_review: None,
            source_system: Some("SNAPTRADE".to_string()),
            source_record_id: st_act.id.clone(),
            source_group_id: None,
            idempotency_key: st_act.id.clone(),
            import_run_id: None,
        };

        activity_upserts.push(activity_upsert);
    }

    // 7. Bulk upsert activities
    let bulk_result = state
        .activity_service
        .upsert_activities_bulk(activity_upserts)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    tracing::info!(
        "SnapTrade sync complete: {} accounts, {} raw activities, {} skipped, {} saved",
        st_accounts.len(),
        raw_activities_count,
        skipped_count,
        bulk_result.upserted
    );

    // 8. Fetch SnapTrade holdings and create snapshots
    let st_holdings = snaptrade::list_holdings(config, &user_id, &user_secret, None)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list holdings: {}", e)))?;

    tracing::info!("SnapTrade returned {} account holdings", st_holdings.len());

    let today_date = Utc::now().date_naive();
    let mut snapshots_to_save: Vec<AccountStateSnapshot> = Vec::new();
    let mut total_positions_count: usize = 0;
    let mut total_cash_count: usize = 0;
    let mut all_asset_ids: HashSet<String> = HashSet::new();
    let mut fx_pairs: HashSet<(String, String)> = HashSet::new();
    // Accounts whose broker /holdings snapshot carried NO positions (e.g. IBKR via
    // SnapTrade returns only cash balances). For these, holdings must come purely
    // from activities, so any stale BrokerImported anchor must be wiped before the
    // recalc (see the clean-slate step below).
    let mut accounts_without_broker_positions: HashSet<String> = HashSet::new();
    let base_currency = state.base_currency.read().unwrap().clone();

    for holding in &st_holdings {
        // Get account ID from holding
        let st_account_id = match holding.account.as_ref().map(|a| a.id.clone()) {
            Some(id) => id,
            None => {
                tracing::warn!("SnapTrade holding skipped: no account in holding");
                continue;
            }
        };

        // Map to WF account ID
        let wf_account_id = match account_map.get(&st_account_id) {
            Some(id) => id.clone(),
            None => {
                tracing::warn!(
                    "SnapTrade holding skipped: account_id {} not in account_map",
                    st_account_id
                );
                continue;
            }
        };

        tracing::info!(
            "Processing holdings for SnapTrade account {} -> WF account {}",
            st_account_id,
            wf_account_id
        );

        // Get account currency
        let account_currency = holding
            .account
            .as_ref()
            .and_then(|a| a.currency.as_ref())
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string());

        // Build positions HashMap
        let mut positions: HashMap<String, Position> = HashMap::new();
        if let Some(ref st_positions) = holding.positions {
            for pos in st_positions {
                // Navigate nested symbol structure: pos.symbol -> SnapTradePositionSymbol -> symbol -> SnapTradeSymbol -> symbol (String)
                let symbol = pos
                    .symbol
                    .as_ref()
                    .and_then(|wrapper| wrapper.symbol.as_ref())
                    .and_then(|inner| inner.symbol.clone())
                    .unwrap_or_else(|| "UNKNOWN".to_string());

                // Get asset description/name from SnapTrade
                let asset_name = pos
                    .symbol
                    .as_ref()
                    .and_then(|wrapper| {
                        wrapper.description.clone().or_else(|| {
                            wrapper.symbol.as_ref().and_then(|inner| inner.description.clone())
                        })
                    });

                let quantity = pos
                    .units
                    .map(|u| Decimal::try_from(u).unwrap_or(Decimal::ZERO))
                    .unwrap_or(Decimal::ZERO);

                let avg_cost = pos
                    .average_purchase_price
                    .map(|p| Decimal::try_from(p).unwrap_or(Decimal::ZERO))
                    .unwrap_or(Decimal::ZERO);

                let total_cost = quantity * avg_cost;

                // Get currency from position level first, then fall back to nested symbol
                let pos_currency = pos
                    .currency
                    .as_ref()
                    .and_then(|c| c.code.clone())
                    .or_else(|| {
                        pos.symbol
                            .as_ref()
                            .and_then(|wrapper| wrapper.symbol.as_ref())
                            .and_then(|inner| inner.currency.as_ref())
                            .and_then(|c| c.code.clone())
                    })
                    .unwrap_or_else(|| account_currency.clone());

                // Build proper AssetMetadata for asset creation
                let metadata = AssetMetadata {
                    name: asset_name.clone(),
                    instrument_symbol: Some(symbol.clone()),
                    display_code: Some(symbol.clone()),
                    requested_quote_ccy: Some(pos_currency.clone()),
                    ..Default::default()
                };

                // Use get_or_create_minimal_asset to ensure asset exists and get its UUID
                let asset = match state
                    .asset_service
                    .get_or_create_minimal_asset(
                        &symbol, // Use symbol as initial asset_id (will be resolved or created)
                        Some(pos_currency.clone()),
                        Some(metadata),
                        None, // Default quote_mode
                    )
                    .await
                {
                    Ok(asset) => asset,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create asset for symbol {}: {}. Skipping position.",
                            symbol,
                            e
                        );
                        continue;
                    }
                };

                tracing::debug!(
                    "Ensured asset for symbol {} -> asset_id {}",
                    symbol,
                    asset.id
                );

                // Track asset ID for quote sync
                all_asset_ids.insert(asset.id.clone());

                // Collect FX pairs for currency conversion
                let asset_ccy = asset.quote_ccy.clone();
                if asset_ccy != account_currency {
                    fx_pairs.insert((asset_ccy.clone(), account_currency.clone()));
                }
                if account_currency != base_currency {
                    fx_pairs.insert((account_currency.clone(), base_currency.clone()));
                }

                let position = Position {
                    id: format!("{}:{}", wf_account_id, asset.id),
                    account_id: wf_account_id.clone(),
                    asset_id: asset.id.clone(),
                    quantity,
                    average_cost: avg_cost,
                    total_cost_basis: total_cost,
                    currency: pos_currency,
                    inception_date: Utc::now(),
                    lots: std::collections::VecDeque::new(),
                    created_at: Utc::now(),
                    last_updated: Utc::now(),
                    is_alternative: false,
                    contract_multiplier: Decimal::ONE,
                };

                positions.insert(asset.id, position);
                total_positions_count += 1;
            }
        }

        // Build cash_balances HashMap
        let mut cash_balances: HashMap<String, Decimal> = HashMap::new();
        if let Some(ref balances) = holding.balances {
            for bal in balances {
                // Holdings API returns cash as direct f64, currency as object
                let currency = bal
                    .currency
                    .as_ref()
                    .and_then(|c| c.code.clone())
                    .unwrap_or_else(|| account_currency.clone());
                let amount = bal
                    .cash
                    .map(|a| Decimal::try_from(a).unwrap_or(Decimal::ZERO))
                    .unwrap_or(Decimal::ZERO);
                cash_balances.insert(currency, amount);
                total_cash_count += 1;
            }
        }

        // Calculate totals
        let cost_basis: Decimal = positions.values().map(|p| p.total_cost_basis).sum();
        let cash_total: Decimal = cash_balances.values().cloned().sum();

        // Create snapshot
        let snapshot = AccountStateSnapshot {
            id: AccountStateSnapshot::stable_id(&wf_account_id, today_date),
            account_id: wf_account_id.clone(),
            snapshot_date: today_date,
            currency: account_currency,
            positions,
            cash_balances,
            cost_basis,
            net_contribution: Decimal::ZERO, // Not available from SnapTrade
            net_contribution_base: Decimal::ZERO,
            cash_total_account_currency: cash_total,
            cash_total_base_currency: cash_total, // Simplified, same as account currency
            calculated_at: Utc::now().naive_utc(),
            source: SnapshotSource::BrokerImported,
        };

        // Only persist a BrokerImported anchor that actually carries positions.
        // These accounts are Transactions-mode, so holdings are derived from the
        // imported activities. A positions-less (cash-only) anchor — e.g. IBKR via
        // SnapTrade returns only cash balances — would otherwise shadow the
        // activity-derived holdings: overwrite_all_snapshots_for_account preserves
        // broker anchors on their date and drops any calculated frame sharing that
        // date, leaving the latest snapshot at 0 positions.
        if snapshot.positions.is_empty() {
            accounts_without_broker_positions.insert(wf_account_id.clone());
            tracing::info!(
                "Skipping positions-less broker anchor for account {} (holdings derived from activities)",
                wf_account_id
            );
        } else {
            snapshots_to_save.push(snapshot);
        }
    }

    // Save all snapshots
    let holdings_synced = snapshots_to_save.len();

    // All WF accounts mapped from SnapTrade this run (deduplicated). Positions for
    // Transactions-mode accounts are derived from activities, so we must recompute
    // per-account holdings from the imported activities. The broker `/holdings`
    // anchor may contain no positions at all (e.g. IBKR via SnapTrade returns only
    // cash balances), and that positions-less anchor would otherwise shadow the
    // real holdings in every view.
    let mut recalc_account_ids: Vec<String> = account_map.values().cloned().collect();
    recalc_account_ids.sort();
    recalc_account_ids.dedup();

    if !snapshots_to_save.is_empty() {
        state
            .snapshot_repository
            .save_snapshots(&snapshots_to_save)
            .await
            .map_err(|e| ApiError::Internal(format!("Failed to save holdings snapshots: {}", e)))?;
        tracing::info!(
            "Saved {} holdings snapshots with {} positions and {} cash balances",
            holdings_synced,
            total_positions_count,
            total_cash_count
        );
    }

    // Recompute per-account holdings from the imported activities and refresh
    // valuations for EVERY mapped account, whether or not a broker anchor was
    // saved above. For Transactions-mode accounts this materializes positions from
    // BUY/SELL activities; the calculated frame at today then becomes the latest
    // snapshot (no positions-less anchor shadows it), so holdings/insights show
    // securities. Position quantities do not need quotes; only their valuation
    // does (handled by the quote/valuation steps below).
    if !recalc_account_ids.is_empty() {
        // Clean slate for accounts with a positions-less broker anchor: remove ALL
        // existing snapshots (any source) for them before recomputing. The broker
        // anchor is normally protected from recalc — overwrite_all_snapshots_for_account
        // and delete_snapshots_by_account_ids only touch source=CALCULATED — so a
        // stale cash-only BrokerImported anchor persisted by an earlier sync would
        // keep shadowing the activity-derived holdings. delete_snapshots_for_account_in_range
        // deletes regardless of source, giving the recalc a clean canvas.
        let wipe_start = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap_or_else(|| today_date - chrono::Duration::days(36500));
        let wipe_end = today_date + chrono::Duration::days(1);
        for account_id in &recalc_account_ids {
            if accounts_without_broker_positions.contains(account_id) {
                if let Err(e) = state
                    .snapshot_repository
                    .delete_snapshots_for_account_in_range(account_id, wipe_start, wipe_end)
                    .await
                {
                    tracing::warn!(
                        "Failed to clear stale snapshots for account {} before recalc: {}",
                        account_id, e
                    );
                } else {
                    tracing::info!(
                        "Cleared stale snapshots for account {} (positions-less broker anchor) before activity recalc",
                        account_id
                    );
                }
            }
        }

        // Register FX pairs for currency conversion (quick, in-request so the
        // background recalc can resolve conversions).
        if !fx_pairs.is_empty() {
            let pairs_vec: Vec<(String, String)> = fx_pairs.into_iter().collect();
            tracing::info!("Registering {} FX pairs for currency conversion...", pairs_vec.len());
            if let Err(e) = state.fx_service.ensure_fx_pairs(pairs_vec).await {
                tracing::warn!("Failed to register FX pairs: {}", e);
            }
        }

        // Delegate the heavy work (market sync + per-account & TOTAL holdings
        // recalculation + valuations) to the shared background portfolio job.
        // Running it synchronously inside this HTTP request made the sync exceed
        // the hosting gateway timeout (Render free tier) and return 502 BEFORE the
        // calculated frame was ever saved — leaving the stale positions-less broker
        // anchor as the latest snapshot, so holdings/insights stayed empty. The job
        // runs off-request (tokio::spawn), rebuilds holdings from the freshly
        // imported activities, and emits SSE progress events the frontend already
        // listens to. The activities and the anchor wipe above are already
        // committed, so the job operates on a clean, complete dataset.
        let market_sync_mode = if all_asset_ids.is_empty() {
            MarketSyncMode::None
        } else {
            let asset_ids_vec: Vec<String> = all_asset_ids.into_iter().collect();
            MarketSyncMode::Incremental {
                asset_ids: Some(asset_ids_vec),
            }
        };
        enqueue_portfolio_job(
            state.clone(),
            PortfolioJobConfig {
                account_ids: Some(recalc_account_ids.clone()),
                market_sync_mode,
                snapshot_mode: SnapshotRecalcMode::Full,
                valuation_mode: ValuationRecalcMode::Full,
            },
        );
        tracing::info!(
            "Enqueued background portfolio recalc for {} account(s) after SnapTrade sync",
            recalc_account_ids.len()
        );
    }

    Ok(Json(SnapTradeSyncResponse {
        accounts_synced: st_accounts.len(),
        activities_synced: bulk_result.upserted,
        raw_activities_from_snaptrade: raw_activities_count,
        skipped_unmapped_account: skipped_count,
        saved: bulk_result.upserted,
        holdings_synced,
        positions_count: total_positions_count,
        cash_balances_count: total_cash_count,
    }))
}

/// GET /api/dfc/snaptrade/sync-diagnostic - Diagnostic endpoint for SnapTrade sync issues
async fn sync_diagnostic(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SyncDiagnosticResponse>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("SnapTrade not configured".to_string()))?;

    let (user_id, user_secret) = get_stored_credentials(&state)?;

    // 1. Fetch SnapTrade accounts
    let st_accounts = snaptrade::list_accounts(config, &user_id, &user_secret)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list accounts: {}", e)))?;

    // 2. Build account info for response
    let snaptrade_accounts: Vec<DiagnosticAccountInfo> = st_accounts
        .iter()
        .map(|a| {
            // Build sync_status string from transactions sync detail
            let sync_status = a.sync_status.as_ref().and_then(|s| {
                s.transactions.as_ref().map(|t| {
                    format!(
                        "initialSyncCompleted: {:?}, lastSync: {}",
                        t.initial_sync_completed.unwrap_or(false),
                        t.last_successful_sync.as_deref().unwrap_or("never")
                    )
                })
            });
            DiagnosticAccountInfo {
                id: a.id.clone(),
                name: a.name.clone(),
                sync_status,
                number: a.number.clone(),
            }
        })
        .collect();

    // 3. Build account_map (snaptrade_id -> wf_id)
    let wf_accounts = state
        .account_service
        .get_active_accounts()
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut account_map: HashMap<String, String> = HashMap::new();
    for st_account in &st_accounts {
        let existing = wf_accounts.iter().find(|a| {
            a.provider.as_deref() == Some("SNAPTRADE")
                && a.provider_account_id.as_deref() == Some(&st_account.id)
        });
        if let Some(wf_account) = existing {
            account_map.insert(st_account.id.clone(), wf_account.id.clone());
        }
    }

    // 4. Compute date range (last 2 years)
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let start = (chrono::Utc::now() - chrono::Duration::days(730))
        .format("%Y-%m-%d")
        .to_string();

    // 5. Fetch SnapTrade activities
    let st_activities = snaptrade::list_activities(config, &user_id, &user_secret, Some(&start), Some(&today))
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to list activities: {}", e)))?;

    // 6. Collect unique account IDs from activities
    let mut activity_account_ids_seen: Vec<String> = st_activities
        .iter()
        .filter_map(|a| a.account.as_ref().and_then(|acc| acc.id.clone()))
        .collect();
    activity_account_ids_seen.sort();
    activity_account_ids_seen.dedup();

    // 7. Find mismatched IDs (in activities but not in accounts)
    let account_ids_set: std::collections::HashSet<&String> = snaptrade_accounts
        .iter()
        .map(|a| &a.id)
        .collect();
    let mismatched_ids: Vec<String> = activity_account_ids_seen
        .iter()
        .filter(|id| !account_ids_set.contains(id))
        .cloned()
        .collect();

    // 8. Per-account DB ground-truth diagnostics (activities + latest snapshot).
    let today_date = chrono::Utc::now().date_naive();
    let mut wf_account_ids: Vec<String> = account_map.values().cloned().collect();
    wf_account_ids.sort();
    wf_account_ids.dedup();

    let mut account_diagnostics: Vec<AccountDiagnostic> = Vec::new();
    for wf_account_id in &wf_account_ids {
        let activities = state
            .activity_service
            .get_activities_by_account_id(wf_account_id)
            .unwrap_or_default();

        let mut activity_type_counts: HashMap<String, usize> = HashMap::new();
        let mut activities_with_asset_id = 0usize;
        let mut activities_without_asset_id = 0usize;
        let mut buy_sell_count = 0usize;
        for activity in &activities {
            *activity_type_counts
                .entry(activity.activity_type.clone())
                .or_insert(0) += 1;
            let has_asset = activity
                .asset_id
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if has_asset {
                activities_with_asset_id += 1;
            } else {
                activities_without_asset_id += 1;
            }
            let upper = activity.activity_type.to_uppercase();
            if upper == "BUY" || upper == "SELL" {
                buy_sell_count += 1;
            }
        }

        let latest_snapshot = state
            .snapshot_repository
            .get_latest_snapshot_before_date(wf_account_id, today_date)
            .ok()
            .flatten();

        let (
            latest_snapshot_date,
            latest_snapshot_source,
            latest_snapshot_positions_count,
            latest_snapshot_raw_positions_count,
            latest_snapshot_position_quantities_sample,
            latest_snapshot_position_asset_ids_sample,
            latest_snapshot_position_assets_resolved,
            latest_snapshot_cash_currencies,
        ) = match latest_snapshot {
            Some(snap) => {
                let raw_positions_count = snap.positions.len();
                let quantities_sample: Vec<String> = snap
                    .positions
                    .values()
                    .take(8)
                    .map(|p| format!("{}={}@{}", p.asset_id, p.quantity, p.currency))
                    .collect();
                let non_zero: Vec<String> = snap
                    .positions
                    .values()
                    .filter(|p| p.quantity != Decimal::ZERO)
                    .map(|p| p.asset_id.clone())
                    .collect();
                let sample: Vec<String> = non_zero.iter().take(5).cloned().collect();
                let cash: Vec<String> = snap.cash_balances.keys().cloned().collect();
                let resolved = if non_zero.is_empty() {
                    0
                } else {
                    state
                        .asset_service
                        .get_assets_by_asset_ids(&non_zero)
                        .await
                        .map(|assets| assets.len())
                        .unwrap_or(0)
                };
                (
                    Some(snap.snapshot_date.to_string()),
                    Some(format!("{:?}", snap.source)),
                    non_zero.len(),
                    raw_positions_count,
                    quantities_sample,
                    sample,
                    resolved,
                    cash,
                )
            }
            None => (None, None, 0, 0, Vec::new(), Vec::new(), 0, Vec::new()),
        };

        // Full snapshot inventory (ground truth): does the recalc engine persist ANY
        // calculated frame for this account, or only the broker anchor?
        let all_snaps = state
            .snapshot_repository
            .get_snapshots_by_account(wf_account_id, None, None)
            .unwrap_or_default();
        let snapshot_total_count = all_snaps.len();
        let mut snapshot_source_counts: HashMap<String, usize> = HashMap::new();
        for s in &all_snaps {
            *snapshot_source_counts
                .entry(format!("{:?}", s.source))
                .or_insert(0) += 1;
        }
        let snapshot_date_min = all_snaps
            .iter()
            .map(|s| s.snapshot_date)
            .min()
            .map(|d| d.to_string());
        let snapshot_date_max = all_snaps
            .iter()
            .map(|s| s.snapshot_date)
            .max()
            .map(|d| d.to_string());
        let latest_calculated = all_snaps
            .iter()
            .filter(|s| format!("{:?}", s.source) == "Calculated")
            .max_by_key(|s| s.snapshot_date);
        let (
            latest_calculated_snapshot_date,
            latest_calculated_raw_positions_count,
            latest_calculated_nonzero_positions_count,
            latest_calculated_quantities_sample,
        ) = match latest_calculated {
            Some(s) => {
                let raw = s.positions.len();
                let nonzero = s
                    .positions
                    .values()
                    .filter(|p| p.quantity != Decimal::ZERO)
                    .count();
                let sample: Vec<String> = s
                    .positions
                    .values()
                    .take(8)
                    .map(|p| format!("{}={}@{}", p.asset_id, p.quantity, p.currency))
                    .collect();
                (Some(s.snapshot_date.to_string()), raw, nonzero, sample)
            }
            None => (None, 0, 0, Vec::new()),
        };

        account_diagnostics.push(AccountDiagnostic {
            wf_account_id: wf_account_id.clone(),
            activity_count: activities.len(),
            activity_type_counts,
            activities_with_asset_id,
            activities_without_asset_id,
            buy_sell_count,
            latest_snapshot_date,
            latest_snapshot_source,
            latest_snapshot_positions_count,
            latest_snapshot_raw_positions_count,
            latest_snapshot_position_quantities_sample,
            latest_snapshot_position_asset_ids_sample,
            latest_snapshot_position_assets_resolved,
            latest_snapshot_cash_currencies,
            snapshot_total_count,
            snapshot_source_counts,
            snapshot_date_min,
            snapshot_date_max,
            latest_calculated_snapshot_date,
            latest_calculated_raw_positions_count,
            latest_calculated_nonzero_positions_count,
            latest_calculated_quantities_sample,
        });
    }

    Ok(Json(SyncDiagnosticResponse {
        build_marker: build_marker(),
        snaptrade_accounts,
        raw_activities_count: st_activities.len(),
        activity_account_ids_seen,
        account_map,
        mismatched_ids,
        date_range: DateRange {
            start,
            end: today,
        },
        account_diagnostics,
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get stored SnapTrade credentials from secret_store
fn get_stored_credentials(state: &AppState) -> ApiResult<(String, String)> {
    let user_id = state
        .secret_store
        .get_secret(SNAPTRADE_USER_ID_KEY)?
        .ok_or_else(|| ApiError::Unauthorized("SnapTrade user not registered".to_string()))?;

    let user_secret = state
        .secret_store
        .get_secret(SNAPTRADE_USER_SECRET_KEY)?
        .ok_or_else(|| ApiError::Unauthorized("SnapTrade user secret not found".to_string()))?;

    Ok((user_id, user_secret))
}

// ============================================================================
// Router
// ============================================================================

/// GET /dfc/snaptrade/debug-sig — temporary diagnostic endpoint
/// Calls SnapTrade's mockSignature endpoint to verify our HMAC is correct.
async fn debug_sig(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    let config = state
        .snaptrade_config
        .as_ref()
        .ok_or_else(|| ApiError::Internal("SnapTrade not configured".to_string()))?;

    let body_value = serde_json::json!({"userId": "dfc-debug"});
    let body_str = serde_json::to_string(&body_value).unwrap();
    let (_base_path, query_string, sig_input, signature) = wealthfolio_core::snaptrade::build_signature(
        config,
        "/snapTrade/registerUser",
        Some(&body_value),
        None,
        None,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Call SnapTrade's own mockSignature endpoint to validate our signature
    let mock_url = format!(
        "https://api.snaptrade.com/api/v1/snapTrade/mockSignature?{}",
        query_string
    );
    let client = reqwest::Client::new();
    let mock_resp = client
        .post(&mock_url)
        .header("Signature", &signature)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("mockSignature request failed: {}", e)))?;

    let mock_status = mock_resp.status().as_u16();
    let mock_body = mock_resp.text().await.unwrap_or_default();

    Ok(Json(serde_json::json!({
        "sigInput": sig_input,
        "signatureEnds": &signature[signature.len().saturating_sub(8)..],
        "sigInputLen": sig_input.len(),
        "mockSignatureStatus": mock_status,
        "mockSignatureResponse": mock_body,
    })))
}

/// GET /dfc/snaptrade/valuation-diagnostic - Diagnostic endpoint for valuation issues
/// Replays the exact day-skip conditions of
/// `ValuationService::calculate_valuation_history` so a diagnostic can show why
/// no valuation row is produced for a date. The calculator only logs these at
/// `debug!` level, which is invisible in production.
fn analyze_valuation_skips(
    state: &Arc<AppState>,
    snapshots: &[AccountStateSnapshot],
) -> serde_json::Value {
    const DAYS_ANALYZED: usize = 15;

    let (Some(first), Some(last)) = (snapshots.first(), snapshots.last()) else {
        return serde_json::json!({ "analyzed": 0, "days": [] });
    };

    let required_asset_ids: HashSet<String> = snapshots
        .iter()
        .flat_map(|snapshot| snapshot.positions.keys().cloned())
        .collect();

    let filled_quotes = state
        .quote_service
        .get_quotes_in_range_filled(&required_asset_ids, first.snapshot_date, last.snapshot_date)
        .unwrap_or_default();

    // An asset absent from this set has no quote anywhere and is valued at zero
    // instead of skipping the day, so it must not count as a gap.
    let mut assets_with_quotes: HashSet<String> = HashSet::new();
    let mut quotes_by_date: HashMap<chrono::NaiveDate, HashMap<String, wealthfolio_core::quotes::Quote>> =
        HashMap::new();
    for quote in &filled_quotes {
        assets_with_quotes.insert(quote.asset_id.clone());
        quotes_by_date
            .entry(quote.timestamp.date_naive())
            .or_default()
            .insert(quote.asset_id.clone(), quote.clone());
    }

    let base_currency = wealthfolio_core::fx::currency::normalize_currency_code(
        &state.base_currency.read().unwrap(),
    )
    .to_string();

    let start = snapshots.len().saturating_sub(DAYS_ANALYZED);
    let days: Vec<serde_json::Value> = snapshots[start..]
        .iter()
        .map(|snapshot| {
            let account_curr =
                wealthfolio_core::fx::currency::normalize_currency_code(&snapshot.currency)
                    .to_string();
            let quotes_for_current_date = quotes_by_date
                .get(&snapshot.snapshot_date)
                .cloned()
                .unwrap_or_default();

            let held: Vec<&String> = snapshot
                .positions
                .iter()
                .filter(|(_, position)| position.quantity != Decimal::ZERO)
                .map(|(asset_id, _)| asset_id)
                .collect();

            let missing: Vec<String> = held
                .iter()
                .filter(|asset_id| assets_with_quotes.contains(**asset_id))
                .filter(|asset_id| {
                    !quotes_for_current_date.contains_key(**asset_id)
                })
                .map(|asset_id| (*asset_id).clone())
                .collect();

            let unquotable = held
                .iter()
                .filter(|asset_id| !assets_with_quotes.contains(**asset_id))
                .count();

            let mut required_fx_pairs: HashSet<(String, String)> = HashSet::new();
            if account_curr != base_currency {
                required_fx_pairs.insert((account_curr.clone(), base_currency.clone()));
            }

            for position in snapshot.positions.values() {
                if position.quantity == Decimal::ZERO {
                    continue;
                }
                let position_currency =
                    wealthfolio_core::fx::currency::normalize_currency_code(&position.currency)
                        .to_string();
                if position_currency != account_curr {
                    required_fx_pairs.insert((position_currency, account_curr.clone()));
                }
            }

            for cash_currency in snapshot.cash_balances.keys() {
                let normalized_cash_currency =
                    wealthfolio_core::fx::currency::normalize_currency_code(cash_currency)
                        .to_string();
                if normalized_cash_currency != account_curr {
                    required_fx_pairs.insert((normalized_cash_currency, account_curr.clone()));
                }
            }

            for quote in quotes_for_current_date.values() {
                let quote_currency =
                    wealthfolio_core::fx::currency::normalize_currency_code(&quote.currency)
                        .to_string();
                if quote_currency != account_curr {
                    required_fx_pairs.insert((quote_currency, account_curr.clone()));
                }
            }

            let mut fx_for_current_date: HashMap<(String, String), Decimal> = HashMap::new();
            let mut missing_fx_pairs: Vec<String> = Vec::new();
            for (from, to) in required_fx_pairs {
                match state
                    .fx_service
                    .get_exchange_rate_for_date(&from, &to, snapshot.snapshot_date)
                {
                    Ok(rate) => {
                        fx_for_current_date.insert((from.clone(), to.clone()), rate);
                    }
                    Err(_) => {
                        missing_fx_pairs.push(format!("{from}->{to}"));
                    }
                }
            }

            let calculation_error = wealthfolio_core::portfolio::valuation::calculate_valuation(
                snapshot,
                &quotes_for_current_date,
                &fx_for_current_date,
                snapshot.snapshot_date,
                &base_currency,
            )
            .err()
            .map(|error| error.to_string());

            let reason = if !missing.is_empty() {
                "quote_gap"
            } else if !missing_fx_pairs.is_empty() {
                "missing_fx_rate"
            } else if calculation_error.is_some() {
                "calculation_error"
            } else {
                "ok"
            };

            serde_json::json!({
                "date": snapshot.snapshot_date.to_string(),
                "accountCurrency": snapshot.currency,
                "heldPositions": held.len(),
                "assetsMissingQuoteForDate": missing.len(),
                "assetsWithNoQuoteAtAll": unquotable,
                "sampleMissingAssetIds": missing.iter().take(5).collect::<Vec<_>>(),
                "fxRateMissing": !missing_fx_pairs.is_empty(),
                "missingFxPairsCount": missing_fx_pairs.len(),
                "sampleMissingFxPairs": missing_fx_pairs.iter().take(5).collect::<Vec<_>>(),
                "calculationError": calculation_error,
                "wouldSkip": reason != "ok",
                "reason": reason,
            })
        })
        .collect();

    serde_json::json!({
        "analyzed": days.len(),
        "assetsWithAnyQuote": assets_with_quotes.len(),
        "assetsRequired": required_asset_ids.len(),
        "days": days,
    })
}

async fn valuation_diagnostic(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    use wealthfolio_core::accounts::AccountServiceTrait;
    
    let accounts = state.account_service.get_active_accounts()
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    
    let today = chrono::Utc::now().date_naive();
    let thirty_days_ago = today - chrono::Duration::days(30);
    
    let mut account_diagnostics = Vec::new();
    
    for account in accounts.iter().filter(|a| a.provider.as_deref() == Some("SNAPTRADE")) {
        // Get snapshots for this account
        let snapshots = state.snapshot_service
            .get_daily_holdings_snapshots(&account.id, Some(thirty_days_ago), Some(today))
            .unwrap_or_default();
        
        // Get valuations for this account
        let valuations = state.valuation_service
            .get_historical_valuations(&account.id, Some(thirty_days_ago), Some(today))
            .unwrap_or_default();
        
        // Collect asset IDs from snapshots
        let mut asset_ids: Vec<String> = Vec::new();
        for snapshot in &snapshots {
            for (asset_id, _) in &snapshot.positions {
                if !asset_ids.contains(asset_id) {
                    asset_ids.push(asset_id.clone());
                }
            }
        }
        
        // Check quotes for each asset
        let mut asset_quote_status = Vec::new();
        for asset_id in &asset_ids {
            // Get asset details
            let asset_info = state.asset_service
                .get_asset_by_id(asset_id)
                .ok()
                .map(|a| serde_json::json!({
                    "id": a.id,
                    "symbol": a.instrument_symbol,
                    "displayCode": a.display_code,
                    "quoteMode": format!("{:?}", a.quote_mode),
                    "quoteCcy": a.quote_ccy,
                }));
            
            // Get latest quote
            let latest_quote = state.quote_service
                .get_latest_quote(asset_id)
                .ok()
                .map(|q| serde_json::json!({
                    "date": q.timestamp.date_naive().to_string(),
                    "close": q.close.to_string(),
                    "currency": q.currency,
                }));
            
            asset_quote_status.push(serde_json::json!({
                "assetId": asset_id,
                "asset": asset_info,
                "latestQuote": latest_quote,
            }));
        }
        
        account_diagnostics.push(serde_json::json!({
            "accountId": account.id,
            "accountName": account.name,
            "snapshotsCount": snapshots.len(),
            "valuationsCount": valuations.len(),
            "assetsCount": asset_ids.len(),
            "assetQuoteStatus": asset_quote_status,
            "valuationSkipAnalysis": analyze_valuation_skips(&state, &snapshots),
            "latestSnapshotDate": snapshots.last().map(|s| s.snapshot_date.to_string()),
            "latestValuationDate": valuations.last().map(|v| v.valuation_date.to_string()),
            "latestValuationValue": valuations.last().map(|v| v.total_value.to_string()),
        }));
    }
    
    // Check FX rates
    let base_currency = state.base_currency.read().unwrap().clone();
    let fx_pairs = vec![
        ("EUR", &base_currency),
        ("GBP", &base_currency),
        ("USD", &base_currency),
    ];
    
    let mut fx_status = Vec::new();
    for (from, to) in fx_pairs {
        let rate = state.fx_service
            .get_exchange_rate_for_date(from, to, today)
            .ok();
        fx_status.push(serde_json::json!({
            "pair": format!("{}/{}", from, to),
            "date": today.to_string(),
            "rate": rate.map(|r| r.to_string()),
        }));
    }
    
    Ok(Json(serde_json::json!({
        "buildMarker": build_marker(),
        "baseCurrency": base_currency,
        "today": today.to_string(),
        "accounts": account_diagnostics,
        "fxRates": fx_status,
    })))
}

/// POST /dfc/assets/enrich - Force enrichment of assets missing profile data
/// 
/// This endpoint finds all assets that have not been enriched (no profile metadata)
/// and triggers the enrichment process to fetch sector, region, and other data from Yahoo.
async fn enrich_unenriched_assets(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    use wealthfolio_core::assets::AssetServiceTrait;
    
    // Get all assets
    let all_assets = state.asset_service.get_assets()
        .map_err(|e| ApiError::Internal(format!("Failed to list assets: {}", e)))?;
    
    // Find assets that need enrichment:
    // - MARKET quote mode (not MANUAL)
    // - No profile metadata (metadata.profile is None or empty)
    // - Has instrument_symbol (can be looked up)
    let unenriched: Vec<String> = all_assets
        .iter()
        .filter(|a| {
            // Only market-mode assets
            a.quote_mode == wealthfolio_core::assets::QuoteMode::Market
            // Has a symbol to look up
            && a.instrument_symbol.is_some()
            // Check if profile is missing or empty
            && a.metadata.as_ref()
                .and_then(|m| m.get("profile"))
                .and_then(|p| p.as_object())
                .map(|obj| obj.is_empty())
                .unwrap_or(true)
        })
        .map(|a| a.id.clone())
        .collect();
    
    let count = unenriched.len();
    tracing::info!("Found {} assets needing enrichment", count);
    
    if unenriched.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": true,
            "message": "All assets already enriched",
            "totalAssets": all_assets.len(),
            "enrichedCount": 0,
        })));
    }
    
    // Spawn enrichment in background to avoid HTTP timeout (takes ~2.5s per asset)
    let asset_service = state.asset_service.clone();
    tokio::spawn(async move {
        match asset_service.enrich_assets(unenriched.clone()).await {
            Ok((enriched, skipped, failed)) => {
                tracing::info!(
                    "Background enrichment complete: {} enriched, {} skipped, {} failed",
                    enriched, skipped, failed
                );
            }
            Err(e) => {
                tracing::error!("Background enrichment failed: {}", e);
            }
        }
    });
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Enrichment started in background for {} assets. Check server logs for progress.", count),
        "totalAssets": all_assets.len(),
        "assetsNeedingEnrichment": count,
    })))
}

/// Purge all SnapTrade snapshots and valuations (to allow clean re-sync)
async fn purge_snaptrade_data(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<serde_json::Value>> {
    use wealthfolio_core::accounts::AccountServiceTrait;
    use chrono::NaiveDate;
    
    // Get all accounts
    let all_accounts = state.account_service.get_all_accounts()
        .map_err(|e| ApiError::Internal(format!("Failed to list accounts: {}", e)))?;
    
    // Find SnapTrade accounts by provider OR by name pattern
    let snaptrade_accounts: Vec<_> = all_accounts
        .iter()
        .filter(|a| {
            a.provider.as_deref() == Some("snaptrade") 
            || a.name.to_lowercase().contains("snaptrade")
        })
        .collect();
    
    let mut purged_accounts = Vec::new();
    
    // Use very wide date range to delete all snapshots
    let far_past = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let far_future = NaiveDate::from_ymd_opt(2100, 12, 31).unwrap();
    
    for account in &snaptrade_accounts {
        // Delete snapshots for this account (using date range)
        let snap_result = state
            .snapshot_repository
            .delete_snapshots_for_account_in_range(&account.id, far_past, far_future)
            .await;
        
        // Delete valuations for this account (None = all dates)
        let val_result = state
            .valuation_repository
            .delete_valuations_for_account(&account.id, None)
            .await;
        
        if snap_result.is_ok() && val_result.is_ok() {
            purged_accounts.push(account.name.clone());
            tracing::info!("Purged all data for SnapTrade account '{}'", account.name);
        } else {
            tracing::warn!(
                "Partial purge for '{}': snapshots={:?}, valuations={:?}",
                account.name, snap_result, val_result
            );
        }
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "SnapTrade data purged. Run /sync to re-import with correct asset IDs.",
        "purgedAccounts": purged_accounts,
    })))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // Diagnostic (remove after signature debugging)
        .route("/dfc/snaptrade/debug-sig", get(debug_sig))
        .route("/dfc/snaptrade/valuation-diagnostic", get(valuation_diagnostic))
        .route("/dfc/snaptrade/purge", post(purge_snaptrade_data))
        // Asset enrichment
        .route("/dfc/assets/enrich", post(enrich_unenriched_assets))
        // Status
        .route("/dfc/snaptrade/status", get(get_status))
        // User registration
        .route("/dfc/snaptrade/register", post(register_user))
        .route("/dfc/snaptrade/user", delete(delete_user))
        // Connections
        .route("/dfc/snaptrade/connect-url", post(get_connect_url))
        .route("/dfc/snaptrade/connections", get(list_connections))
        .route("/dfc/snaptrade/connections/{auth_id}", delete(delete_connection))
        // Accounts
        .route("/dfc/snaptrade/accounts", get(list_accounts))
        // Sync
        .route("/dfc/snaptrade/sync", post(sync_snaptrade))
        .route("/dfc/snaptrade/sync-diagnostic", get(sync_diagnostic))
}
