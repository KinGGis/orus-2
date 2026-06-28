use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::quotes::{
    DataSource, MarketDataProviderSetting, ProviderCapabilities, Quote, QuoteSyncState,
};
use wealthfolio_core::{Error, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(
    Queryable,
    Identifiable,
    Selectable,
    Insertable,
    AsChangeset,
    Debug,
    Clone,
)]
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
    #[diesel(column_name = timestamp_)]
    pub timestamp: DateTime<Utc>,
}

#[derive(
    Debug,
    Clone,
    Queryable,
    Identifiable,
    Selectable,
    Insertable,
    AsChangeset,
)]
#[diesel(table_name = crate::schema::wf_market_data_providers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MarketDataProviderSettingDB {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: Option<String>,
    pub priority: i32,
    pub enabled: bool,
    pub logo_filename: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_sync_status: Option<String>,
    pub last_sync_error: Option<String>,
}

#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = crate::schema::wf_market_data_providers)]
pub struct UpdateMarketDataProviderSettingDB {
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
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

fn stable_quote_uuid(quote: &Quote) -> Uuid {
    if let Ok(id) = Uuid::parse_str(&quote.id) {
        return id;
    }

    Uuid::new_v4()
}

fn parse_quote_asset_id(asset_id: &str) -> Result<Uuid> {
    Uuid::parse_str(asset_id).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid quote asset_id UUID: {err}"
        )))
    })
}

impl From<QuoteDB> for Quote {
    fn from(db: QuoteDB) -> Self {
        Quote {
            id: db.id.to_string(),
            asset_id: db.asset_id.to_string(),
            timestamp: db.timestamp,
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

impl TryFrom<&Quote> for QuoteDB {
    type Error = Error;

    fn try_from(quote: &Quote) -> Result<Self> {
        Ok(Self {
            id: stable_quote_uuid(quote),
            asset_id: parse_quote_asset_id(&quote.asset_id)?,
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
            timestamp: quote.timestamp,
        })
    }
}

impl From<MarketDataProviderSettingDB> for MarketDataProviderSetting {
    fn from(db: MarketDataProviderSettingDB) -> Self {
        let capabilities = ProviderCapabilities::for_provider(&db.id);
        Self {
            id: db.id,
            name: db.name,
            description: db.description,
            url: db.url,
            priority: db.priority,
            enabled: db.enabled,
            logo_filename: db.logo_filename,
            last_synced_at: db.last_synced_at.map(|dt| dt.to_rfc3339()),
            last_sync_status: db.last_sync_status,
            last_sync_error: db.last_sync_error,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Queryable, Identifiable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = crate::schema::wf_quote_sync_state)]
#[diesel(primary_key(asset_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct QuoteSyncStateDB {
    pub asset_id: Uuid,
    pub position_closed_date: Option<NaiveDate>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub data_source: String,
    pub sync_priority: i32,
    pub error_count: i32,
    pub last_error: Option<String>,
    pub profile_enriched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = crate::schema::wf_quote_sync_state)]
pub struct QuoteSyncStateUpdateDB {
    pub position_closed_date: Option<Option<NaiveDate>>,
    pub last_synced_at: Option<Option<DateTime<Utc>>>,
    pub data_source: Option<String>,
    pub sync_priority: Option<i32>,
    pub error_count: Option<i32>,
    pub last_error: Option<Option<String>>,
    pub profile_enriched_at: Option<Option<DateTime<Utc>>>,
    pub updated_at: Option<DateTime<Utc>>,
}

fn parse_asset_id(asset_id: &str) -> Result<Uuid> {
    Uuid::parse_str(asset_id).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid asset_id UUID: {err}"
        )))
    })
}

impl From<QuoteSyncStateDB> for QuoteSyncState {
    fn from(db: QuoteSyncStateDB) -> Self {
        QuoteSyncState {
            asset_id: db.asset_id.to_string(),
            is_active: db.position_closed_date.is_none(),
            position_closed_date: db.position_closed_date,
            last_synced_at: db.last_synced_at,
            data_source: db.data_source,
            sync_priority: db.sync_priority,
            error_count: db.error_count,
            last_error: db.last_error,
            profile_enriched_at: db.profile_enriched_at,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl QuoteSyncStateDB {
    pub fn from_domain(state: &QuoteSyncState) -> Result<Self> {
        Ok(Self {
            asset_id: parse_asset_id(&state.asset_id)?,
            position_closed_date: state.position_closed_date,
            last_synced_at: state.last_synced_at,
            data_source: state.data_source.clone(),
            sync_priority: state.sync_priority,
            error_count: state.error_count,
            last_error: state.last_error.clone(),
            profile_enriched_at: state.profile_enriched_at,
            created_at: state.created_at,
            updated_at: state.updated_at,
        })
    }
}

pub fn parse_quote_sync_asset_id(asset_id: &str) -> Result<Uuid> {
    parse_asset_id(asset_id)
}