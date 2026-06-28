use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use wealthfolio_core::assets::{Asset, AssetKind, InstrumentType, NewAsset, QuoteMode};

#[derive(Queryable, Identifiable, Selectable, Insertable, AsChangeset, Debug, Clone)]
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

#[derive(Insertable, AsChangeset, Debug, Clone)]
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

impl From<AssetDB> for Asset {
    fn from(db: AssetDB) -> Self {
        Self {
            id: db.id.to_string(),
            kind: AssetKind::from_db_str(&db.kind).unwrap_or_default(),
            name: db.name,
            display_code: db.display_code,
            notes: db.notes,
            metadata: db.metadata,
            is_active: db.is_active,
            quote_mode: match db.quote_mode.as_str() {
                "MANUAL" => QuoteMode::Manual,
                "INTERNAL_YTM" => QuoteMode::InternalYtm,
                _ => QuoteMode::Market,
            },
            quote_ccy: db.quote_ccy,
            instrument_type: db
                .instrument_type
                .as_deref()
                .and_then(InstrumentType::from_db_str),
            instrument_symbol: db.instrument_symbol,
            instrument_exchange_mic: db.instrument_exchange_mic,
            instrument_key: db.instrument_key,
            provider_config: db.provider_config,
            exchange_name: None,
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
        }
    }
}

impl From<NewAsset> for InsertableAssetDB {
    fn from(domain: NewAsset) -> Self {
        let now = Utc::now();
        Self {
            id: domain
                .id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .unwrap_or_else(Uuid::new_v4),
            kind: domain.kind.as_db_str().to_string(),
            name: domain.name,
            display_code: domain.display_code,
            notes: domain.notes,
            metadata: domain.metadata,
            is_active: domain.is_active,
            quote_mode: domain.quote_mode.as_db_str().to_string(),
            quote_ccy: domain.quote_ccy,
            instrument_type: domain
                .instrument_type
                .as_ref()
                .map(|value| value.as_db_str().to_string()),
            instrument_symbol: domain.instrument_symbol,
            instrument_exchange_mic: domain.instrument_exchange_mic,
            provider_config: domain.provider_config,
            created_at: now,
            updated_at: now,
        }
    }
}