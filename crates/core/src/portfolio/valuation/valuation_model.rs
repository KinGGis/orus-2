//! Portfolio valuation domain models.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use uuid::Uuid;

/// Domain model for daily account valuation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyAccountValuation {
    pub id: String,
    pub account_id: String,
    pub valuation_date: NaiveDate,
    pub account_currency: String,
    pub base_currency: String,
    pub fx_rate_to_base: Decimal,
    pub cash_balance: Decimal,
    pub investment_market_value: Decimal,
    pub total_value: Decimal,
    pub cost_basis: Decimal,
    pub net_contribution: Decimal,
    pub calculated_at: DateTime<Utc>,
}

impl DailyAccountValuation {
    /// Deterministic UUID derived from account/date, so backends that store the
    /// id as a UUID can upsert the same row across recalculations.
    ///
    /// Mirrors `AccountStateSnapshot::stable_id`; the namespace differs so a
    /// valuation and a snapshot for the same account/date never collide.
    pub fn stable_id(account_id: &str, valuation_date: NaiveDate) -> String {
        let name = format!(
            "wealthfolio:valuation:{}:{}",
            account_id,
            valuation_date.format("%Y-%m-%d")
        );
        let digest = sha2::Sha256::digest(name.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        // RFC4122 variant + version 5 bit layout.
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    }
}
