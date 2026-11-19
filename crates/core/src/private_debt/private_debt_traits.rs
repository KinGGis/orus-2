use async_trait::async_trait;
use chrono::NaiveDate;

use super::private_debt_model::{
    DebtFlowSettlement, DebtScheduleFlow, NewDebtScheduleFlow, RateFixing,
};
use crate::errors::Result;

#[async_trait]
pub trait DebtScheduleRepositoryTrait: Send + Sync {
    /// The schedule of one loan held in one account, ordered by date.
    fn list_flows(&self, account_id: &str, asset_id: &str) -> Result<Vec<DebtScheduleFlow>>;

    /// Flows of the whole book falling due in `[from, to]`, ordered by date.
    fn list_flows_due(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<DebtScheduleFlow>>;

    /// Replaces the still-expected part of a loan's schedule with `flows`.
    ///
    /// Flows already settled (any status other than EXPECTED, or linked to an
    /// activity) are kept, and a regenerated flow falling on the same date and
    /// type as one of them is dropped, so re-running the engine after a rate
    /// fixing or a term change never erases what was reconciled.
    /// Returns the number of flows inserted.
    async fn replace_expected_flows(
        &self,
        account_id: &str,
        asset_id: &str,
        flows: Vec<NewDebtScheduleFlow>,
    ) -> Result<usize>;

    async fn settle_flow(
        &self,
        flow_id: &str,
        settlement: DebtFlowSettlement,
    ) -> Result<DebtScheduleFlow>;
}

#[async_trait]
pub trait RateFixingRepositoryTrait: Send + Sync {
    /// Latest fixing of `index_code` published on or before `date`.
    fn fixing_on(&self, index_code: &str, date: NaiveDate) -> Result<Option<RateFixing>>;

    /// Inserts fixings, overwriting any already stored for the same index and day.
    /// Returns the number of fixings written.
    async fn upsert_fixings(&self, fixings: Vec<RateFixing>) -> Result<usize>;
}
