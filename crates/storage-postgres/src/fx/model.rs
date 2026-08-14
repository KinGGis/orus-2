use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use wealthfolio_core::quotes::{DataSource, Quote};

#[derive(Queryable, Selectable, Identifiable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::wf_assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AssetDB {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub display_code: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<Value>,
    pub is_active: bool,
    pub quote_mode: String,
    pub quote_ccy: String,
    pub instrument_type: Option<String>,
    pub instrument_symbol: Option<String>,
    pub instrument_exchange_mic: Option<String>,
    pub instrument_key: Option<String>,
    pub provider_config: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// `instrument_key` is intentionally absent: the column is GENERATED ALWAYS in
// Postgres, so any explicit value makes the INSERT fail. The database derives
// `FX:<instrument_symbol>/<quote_ccy>`, which matches
// `ExchangeRate::make_instrument_key`.
#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_assets)]
pub struct InsertableAssetDB {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub display_code: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<Value>,
    pub is_active: bool,
    pub quote_mode: String,
    pub quote_ccy: String,
    pub instrument_type: Option<String>,
    pub instrument_symbol: Option<String>,
    pub instrument_exchange_mic: Option<String>,
    pub provider_config: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Queryable, Selectable, Identifiable, Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_quotes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct QuoteDB {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub day: NaiveDate,
    pub source: String,
    pub open: Option<BigDecimal>,
    pub high: Option<BigDecimal>,
    pub low: Option<BigDecimal>,
    pub close: BigDecimal,
    pub adjclose: Option<BigDecimal>,
    pub volume: Option<i64>,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub timestamp_: DateTime<Utc>,
}

fn decimal_from_bigdecimal(value: &BigDecimal) -> Decimal {
    Decimal::from_str(&value.to_string()).unwrap_or_default()
}

fn optional_decimal_from_bigdecimal(value: &Option<BigDecimal>) -> Decimal {
    value.as_ref().map(decimal_from_bigdecimal).unwrap_or_default()
}

fn bigdecimal_from_decimal(value: &Decimal) -> BigDecimal {
    BigDecimal::from_str(&value.to_string()).unwrap_or_else(|_| BigDecimal::from(0))
}

impl From<QuoteDB> for Quote {
    fn from(db: QuoteDB) -> Self {
        Quote {
            id: db.id.to_string(),
            asset_id: db.asset_id.to_string(),
            timestamp: db.timestamp_,
            open: optional_decimal_from_bigdecimal(&db.open),
            high: optional_decimal_from_bigdecimal(&db.high),
            low: optional_decimal_from_bigdecimal(&db.low),
            close: decimal_from_bigdecimal(&db.close),
            adjclose: optional_decimal_from_bigdecimal(&db.adjclose),
            volume: db.volume.map(Decimal::from).unwrap_or_default(),
            currency: db.currency,
            data_source: DataSource::from(db.source.as_str()),
            created_at: db.created_at,
            notes: db.notes,
        }
    }
}

impl QuoteDB {
    pub fn from_quote(quote: &Quote) -> Result<Self, uuid::Error> {
        Ok(Self {
            id: if quote.id.is_empty() {
                Uuid::new_v4()
            } else {
                Uuid::parse_str(&quote.id)?
            },
            asset_id: Uuid::parse_str(&quote.asset_id)?,
            day: quote.timestamp.date_naive(),
            source: quote.data_source.as_str().to_string(),
            open: if quote.open.is_zero() {
                None
            } else {
                Some(bigdecimal_from_decimal(&quote.open))
            },
            high: if quote.high.is_zero() {
                None
            } else {
                Some(bigdecimal_from_decimal(&quote.high))
            },
            low: if quote.low.is_zero() {
                None
            } else {
                Some(bigdecimal_from_decimal(&quote.low))
            },
            close: bigdecimal_from_decimal(&quote.close),
            adjclose: if quote.adjclose.is_zero() {
                None
            } else {
                Some(bigdecimal_from_decimal(&quote.adjclose))
            },
            volume: if quote.volume.is_zero() {
                None
            } else {
                quote.volume.to_i64()
            },
            currency: quote.currency.clone(),
            notes: quote.notes.clone(),
            created_at: quote.created_at,
            timestamp_: quote.timestamp,
        })
    }
}