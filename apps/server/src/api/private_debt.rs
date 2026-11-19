use std::sync::Arc;

use crate::{api::shared::trigger_full_portfolio_recalc, error::ApiResult, main_lib::AppState};
use axum::{extract::State, routing::post, Json, Router};
use wealthfolio_core::private_debt::{
    import_private_debt, PrivateDebtImportRequest, PrivateDebtImportSummary,
};

async fn import_statement(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PrivateDebtImportRequest>,
) -> ApiResult<Json<PrivateDebtImportSummary>> {
    let summary = import_private_debt(
        state.asset_service.as_ref(),
        state.activity_service.as_ref(),
        state.taxonomy_service.as_ref(),
        request,
    )
    .await?;
    // The full job also refreshes the loans' schedules, so the imported lines
    // confirm their estimated flows right away.
    if summary.loans_created + summary.loans_updated + summary.activities_created > 0 {
        trigger_full_portfolio_recalc(state);
    }
    Ok(Json(summary))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/private-debt/import", post(import_statement))
}
