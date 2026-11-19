use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DebtFlowType {
    /// Interest paid out in cash.
    InterestCash,
    /// Interest capitalised into principal.
    InterestPik,
    /// Principal repayment (amortisation or bullet).
    Principal,
    /// Fee charged to the lender.
    Fee,
}

impl DebtFlowType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InterestCash => "INTEREST_CASH",
            Self::InterestPik => "INTEREST_PIK",
            Self::Principal => "PRINCIPAL",
            Self::Fee => "FEE",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "INTEREST_CASH" => Ok(Self::InterestCash),
            "INTEREST_PIK" => Ok(Self::InterestPik),
            "PRINCIPAL" => Ok(Self::Principal),
            "FEE" => Ok(Self::Fee),
            other => Err(invalid("flow type", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DebtFlowStatus {
    /// Scheduled, not booked yet.
    Expected,
    /// Due and booked from the schedule; counted in the valuation until the
    /// platform statement confirms it.
    Estimated,
    /// Matched with a real activity.
    Confirmed,
    Partial,
    Late,
    Waived,
}

impl DebtFlowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expected => "EXPECTED",
            Self::Estimated => "ESTIMATED",
            Self::Confirmed => "CONFIRMED",
            Self::Partial => "PARTIAL",
            Self::Late => "LATE",
            Self::Waived => "WAIVED",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "EXPECTED" => Ok(Self::Expected),
            "ESTIMATED" => Ok(Self::Estimated),
            "CONFIRMED" => Ok(Self::Confirmed),
            "PARTIAL" => Ok(Self::Partial),
            "LATE" => Ok(Self::Late),
            "WAIVED" => Ok(Self::Waived),
            other => Err(invalid("flow status", other)),
        }
    }
}

fn invalid(what: &str, value: &str) -> Error {
    Error::Validation(ValidationError::InvalidInput(format!(
        "Unknown debt {what}: {value}"
    )))
}

/// One expected cash flow of a private loan, and how it was settled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtScheduleFlow {
    pub id: String,
    pub account_id: String,
    pub asset_id: String,
    pub flow_date: NaiveDate,
    pub flow_type: DebtFlowType,
    pub period_start: Option<NaiveDate>,
    pub expected_amount: Decimal,
    pub currency: String,
    pub rate_applied: Option<Decimal>,
    pub principal_before: Option<Decimal>,
    pub status: DebtFlowStatus,
    pub received_amount: Option<Decimal>,
    pub received_date: Option<NaiveDate>,
    pub activity_id: Option<String>,
    pub notes: Option<String>,
}

/// A flow produced by the schedule engine, before it is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDebtScheduleFlow {
    pub flow_date: NaiveDate,
    pub flow_type: DebtFlowType,
    pub period_start: Option<NaiveDate>,
    pub expected_amount: Decimal,
    pub currency: String,
    pub rate_applied: Option<Decimal>,
    pub principal_before: Option<Decimal>,
}

/// Settlement of an expected flow against what was actually received.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtFlowSettlement {
    pub status: DebtFlowStatus,
    pub received_amount: Option<Decimal>,
    pub received_date: Option<NaiveDate>,
    pub activity_id: Option<String>,
    pub notes: Option<String>,
}

/// Published value of a floating-rate index on a given day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateFixing {
    pub index_code: String,
    pub fixing_date: NaiveDate,
    /// Annual rate as a decimal (0.0215 for 2.15%).
    pub rate: Decimal,
    pub source: String,
}
