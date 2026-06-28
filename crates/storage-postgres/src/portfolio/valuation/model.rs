use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use crate::system_accounts::{account_id_to_domain, parse_account_id};
use wealthfolio_core::constants::DECIMAL_PRECISION;
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::portfolio::valuation::DailyAccountValuation;
use wealthfolio_core::{Error, Result};

#[derive(Queryable, Identifiable, Selectable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_daily_account_valuation)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DailyAccountValuationDB {
    pub id: Uuid,
    pub account_id: Uuid,
    pub valuation_date: NaiveDate,
    pub account_currency: String,
    pub base_currency: String,
    pub fx_rate_to_base: BigDecimal,
    pub cash_balance: BigDecimal,
    pub investment_market_value: BigDecimal,
    pub total_value: BigDecimal,
    pub cost_basis: BigDecimal,
    pub net_contribution: BigDecimal,
    pub calculated_at: DateTime<Utc>,
}

fn decimal_from_bigdecimal(value: &BigDecimal) -> Decimal {
    Decimal::from_str(&value.to_string()).unwrap_or_default()
}

fn bigdecimal_from_decimal(value: Decimal) -> BigDecimal {
    BigDecimal::from_str(&value.round_dp(DECIMAL_PRECISION).to_string())
        .unwrap_or_else(|_| BigDecimal::from(0))
}

impl From<DailyAccountValuationDB> for DailyAccountValuation {
    fn from(value: DailyAccountValuationDB) -> Self {
        Self {
            id: value.id.to_string(),
            account_id: account_id_to_domain(value.account_id),
            valuation_date: value.valuation_date,
            account_currency: value.account_currency,
            base_currency: value.base_currency,
            fx_rate_to_base: decimal_from_bigdecimal(&value.fx_rate_to_base),
            cash_balance: decimal_from_bigdecimal(&value.cash_balance),
            investment_market_value: decimal_from_bigdecimal(&value.investment_market_value),
            total_value: decimal_from_bigdecimal(&value.total_value),
            cost_basis: decimal_from_bigdecimal(&value.cost_basis),
            net_contribution: decimal_from_bigdecimal(&value.net_contribution),
            calculated_at: value.calculated_at,
        }
    }
}

impl TryFrom<&DailyAccountValuation> for DailyAccountValuationDB {
    type Error = Error;

    fn try_from(value: &DailyAccountValuation) -> Result<Self> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid valuation id UUID: {err}"
                )))
            })?,
            account_id: parse_account_id(&value.account_id)?,
            valuation_date: value.valuation_date,
            account_currency: value.account_currency.clone(),
            base_currency: value.base_currency.clone(),
            fx_rate_to_base: bigdecimal_from_decimal(value.fx_rate_to_base),
            cash_balance: bigdecimal_from_decimal(value.cash_balance),
            investment_market_value: bigdecimal_from_decimal(value.investment_market_value),
            total_value: bigdecimal_from_decimal(value.total_value),
            cost_basis: bigdecimal_from_decimal(value.cost_basis),
            net_contribution: bigdecimal_from_decimal(value.net_contribution),
            calculated_at: value.calculated_at,
        })
    }
}