//! Private debt - expected cash-flow schedules of private loans and the
//! floating-rate fixings they are computed from.
//!
//! A loan itself is a BOND asset; its terms live in [`crate::assets::BondSpec`].

mod private_debt_model;
mod private_debt_traits;

pub use private_debt_model::{
    DebtFlowSettlement, DebtFlowStatus, DebtFlowType, DebtScheduleFlow, NewDebtScheduleFlow,
    RateFixing,
};
pub use private_debt_traits::{DebtScheduleRepositoryTrait, RateFixingRepositoryTrait};
