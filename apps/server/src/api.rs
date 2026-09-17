use std::sync::Arc;

use crate::{
    auth,
    config::Config,
    main_lib::AppState,
    models::{Account, AccountUpdate, NewAccount},
};
use axum::middleware;
use axum::response::IntoResponse;
use axum::{routing::get, Json, Router};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;

mod accounts;
mod activities;
mod addons;
mod ai_chat;
mod ai_providers;
mod alternative_assets;
mod assets;
#[cfg(any(feature = "connect-sync", feature = "device-sync"))]
pub mod connect;
#[cfg(feature = "device-sync")]
mod device_sync;
#[cfg(feature = "device-sync")]
pub(crate) mod device_sync_engine;
mod exchange_rates;
mod goals;
mod health;
mod holdings;
mod limits;
mod market_data;
mod net_worth;
mod performance;
mod portfolio;
mod revolut;
mod secrets;
mod settings;
mod snaptrade;
pub mod shared;
#[cfg(feature = "device-sync")]
mod sync_crypto;
mod taxonomies;
mod users;

#[utoipa::path(get, path = "/api/v1/healthz", responses((status = 200, description = "Health")))]
pub async fn healthz() -> &'static str {
    "ok"
}

#[utoipa::path(get, path = "/api/v1/readyz", responses((status = 200, description = "Ready")))]
pub async fn readyz() -> &'static str {
    "ok"
}

#[derive(OpenApi)]
#[openapi(
    paths(healthz, readyz, accounts::list_accounts, accounts::create_account, accounts::update_account, accounts::delete_account),
    components(schemas(Account, NewAccount, AccountUpdate)),
    tags((name="wealthfolio"))
)]
pub struct ApiDoc;

#[allow(deprecated)]
pub fn app_router(state: Arc<AppState>, config: &Config) -> Router {
    let cors = if config.cors_allow.iter().any(|o| o == "*") {
        CorsLayer::new().allow_origin(Any)
    } else {
        let origins = config
            .cors_allow
            .iter()
            .map(|o| o.parse().unwrap())
            .collect::<Vec<_>>();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_credentials(true)
    };

    let openapi = ApiDoc::openapi();
    let requires_auth = state.auth.is_some() || state.orus_auth.is_some();

    // Compose all protected routes from individual modules
    #[allow(unused_mut)]
    let mut protected_api = Router::new()
        .merge(accounts::router())
        .merge(settings::router())
        .merge(portfolio::router())
        .merge(holdings::router())
        .merge(performance::router())
        .merge(activities::router())
        .merge(goals::router())
        .merge(exchange_rates::router())
        .merge(market_data::router())
        .merge(assets::router())
        .merge(secrets::router())
        .merge(limits::router())
        .merge(addons::router())
        .merge(taxonomies::router())
        .merge(net_worth::router())
        .merge(alternative_assets::router())
        .merge(ai_providers::router())
        .merge(ai_chat::router())
        .merge(health::router())
        // DFC-specific routes
        .merge(users::router())
        .merge(revolut::router())
        .merge(snaptrade::router());

    #[cfg(feature = "device-sync")]
    {
        protected_api = protected_api
            .merge(device_sync::router())
            .merge(sync_crypto::router());
    }

    #[cfg(any(feature = "connect-sync", feature = "device-sync"))]
    {
        protected_api = protected_api.merge(connect::router());
    }

    let protected_api = protected_api.route(
        "/openapi.json",
        get({
            let openapi = openapi.clone();
            move || async { Json(openapi) }
        }),
    );

    let protected_api = if requires_auth {
        protected_api.layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_jwt,
        ))
    } else {
        protected_api
    };

    // Rate limit login: 5 requests per 60 seconds per peer IP
    let login_governor = GovernorConfigBuilder::default()
        .per_second(12) // replenish 1 token every 12s → 5 per 60s
        .burst_size(5)
        .finish()
        .expect("valid governor config");

    let default_timeout = config.request_timeout;
    let long_timeout = config.long_request_timeout;

    let api = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/auth/status", get(auth::auth_status))
        .route("/auth/orus/session", axum::routing::post(auth::establish_orus_session))
        .route(
            "/auth/login",
            axum::routing::post(auth::login).layer(GovernorLayer::new(login_governor)),
        )
        .route("/auth/logout", axum::routing::post(auth::logout))
        .route("/auth/me", get(auth::auth_me))
        .merge(protected_api)
        .with_state(state.clone());

    Router::new()
        .nest("/api/v1", api)
        .with_state(state)
        .layer(cors)
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(middleware::from_fn(move |req: axum::extract::Request, next: middleware::Next| {
            // A single global timeout cannot serve both interactive endpoints and
            // broker syncs: the latter chain many upstream calls and legitimately
            // run for minutes, and were being cut off with 408 Request Timeout.
            let budget = if is_long_running_path(req.uri().path()) {
                long_timeout
            } else {
                default_timeout
            };
            async move {
                match tokio::time::timeout(budget, next.run(req)).await {
                    Ok(response) => response,
                    Err(_) => (
                        axum::http::StatusCode::REQUEST_TIMEOUT,
                        "Request timed out",
                    )
                        .into_response(),
                }
            }
        }))        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        path = %request.uri().path(),
                    )
                })
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
}

/// Endpoints that drive a full broker synchronisation. They fan out to many
/// upstream provider calls before committing, so they get the long timeout
/// budget instead of the interactive one.
fn is_long_running_path(path: &str) -> bool {
    path.ends_with("/snaptrade/sync")
        || path.ends_with("/snaptrade/sync-diagnostic")
        || path.ends_with("/revolut/sync")
        || path.ends_with("/connect/sync")
}