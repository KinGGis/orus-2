use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::system_accounts::{account_id_to_domain, parse_account_id};
use wealthfolio_core::constants::DECIMAL_PRECISION;
use wealthfolio_core::portfolio::snapshot::{AccountStateSnapshot, SnapshotSource};
use wealthfolio_core::{Error, Result};
use wealthfolio_core::errors::ValidationError;

#[derive(Queryable, Identifiable, Selectable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_holdings_snapshots)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountStateSnapshotDB {
    pub id: Uuid,
    pub account_id: Uuid,
    pub snapshot_date: NaiveDate,
    pub currency: String,
    pub positions: Value,
    pub cash_balances: Value,
    pub cost_basis: BigDecimal,
    pub net_contribution: BigDecimal,
    pub calculated_at: DateTime<Utc>,
    pub net_contribution_base: BigDecimal,
    pub cash_total_account_currency: BigDecimal,
    pub cash_total_base_currency: BigDecimal,
    pub source: String,
}

fn decimal_from_bigdecimal(value: &BigDecimal) -> Decimal {
    Decimal::from_str(&value.to_string()).unwrap_or_default()
}

fn bigdecimal_from_decimal(value: Decimal) -> BigDecimal {
    BigDecimal::from_str(&value.round_dp(DECIMAL_PRECISION).to_string())
        .unwrap_or_else(|_| BigDecimal::from(0))
}

fn source_from_string(value: &str) -> SnapshotSource {
    serde_json::from_str(&format!("\"{value}\"")).unwrap_or(SnapshotSource::Calculated)
}

impl From<AccountStateSnapshotDB> for AccountStateSnapshot {
    fn from(db: AccountStateSnapshotDB) -> Self {
        Self {
            id: db.id.to_string(),
            account_id: account_id_to_domain(db.account_id),
            snapshot_date: db.snapshot_date,
            currency: db.currency,
            positions: serde_json::from_value(db.positions).unwrap_or_default(),
            cash_balances: serde_json::from_value(db.cash_balances).unwrap_or_default(),
            cost_basis: decimal_from_bigdecimal(&db.cost_basis),
            net_contribution: decimal_from_bigdecimal(&db.net_contribution),
            net_contribution_base: decimal_from_bigdecimal(&db.net_contribution_base),
            cash_total_account_currency: decimal_from_bigdecimal(&db.cash_total_account_currency),
            cash_total_base_currency: decimal_from_bigdecimal(&db.cash_total_base_currency),
            calculated_at: db.calculated_at.naive_utc(),
            source: source_from_string(&db.source),
        }
    }
}

impl TryFrom<&AccountStateSnapshot> for AccountStateSnapshotDB {
    type Error = Error;

    fn try_from(domain: &AccountStateSnapshot) -> Result<Self> {
        let stable_id = if domain.id.trim().is_empty() {
            AccountStateSnapshot::stable_id(&domain.account_id, domain.snapshot_date)
        } else {
            domain.id.clone()
        };

        let calculated_at = DateTime::<Utc>::from_naive_utc_and_offset(domain.calculated_at, Utc);

        // Postgres stores the snapshot id as a UUID. Domain ids may be
        // non-UUID stable identifiers (e.g. the synthetic "TOTAL" portfolio
        // snapshot). Fall back to the deterministic UUID derived from
        // account_id + snapshot_date whenever the provided id is not a UUID so
        // upserts still target a stable row.
        let snapshot_id = Uuid::parse_str(&stable_id).or_else(|_| {
            Uuid::parse_str(&AccountStateSnapshot::stable_id(
                &domain.account_id,
                domain.snapshot_date,
            ))
        });

        Ok(Self {
            id: snapshot_id.map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid snapshot id UUID: {err}"
                )))
            })?,
            account_id: parse_account_id(&domain.account_id)?,
            snapshot_date: domain.snapshot_date,
            currency: domain.currency.clone(),
            positions: serde_json::to_value(&domain.positions).map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid snapshot positions JSON: {err}"
                )))
            })?,
            cash_balances: serde_json::to_value(&domain.cash_balances).map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid snapshot cash_balances JSON: {err}"
                )))
            })?,
            cost_basis: bigdecimal_from_decimal(domain.cost_basis),
            net_contribution: bigdecimal_from_decimal(domain.net_contribution),
            calculated_at,
            net_contribution_base: bigdecimal_from_decimal(domain.net_contribution_base),
            cash_total_account_currency: bigdecimal_from_decimal(domain.cash_total_account_currency),
            cash_total_base_currency: bigdecimal_from_decimal(domain.cash_total_base_currency),
            source: serde_json::to_string(&domain.source)
                .unwrap_or_else(|_| "\"CALCULATED\"".to_string())
                .trim_matches('"')
                .to_string(),
        })
    }
}