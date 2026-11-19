//! Private debt - expected cash-flow schedules of private loans and the
//! floating-rate fixings they are computed from.
//!
//! A loan itself is a BOND asset; its terms live in [`crate::assets::BondSpec`].

mod debt_sync;
mod private_debt_model;
mod private_debt_traits;
mod schedule_engine;

pub use debt_sync::{sync_private_debt, DebtSyncSummary, GENERATED_KEY_PREFIX};
pub use private_debt_model::{
    DebtFlowSettlement, DebtFlowStatus, DebtFlowType, DebtScheduleFlow, NewDebtScheduleFlow,
    RateFixing,
};
pub use private_debt_traits::{DebtScheduleRepositoryTrait, RateFixingRepositoryTrait};
pub use schedule_engine::build_schedule;
