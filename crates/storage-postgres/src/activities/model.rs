use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use wealthfolio_core::activities::{
    parse_decimal_string_tolerant, Activity, ActivityStatus, ActivityUpdate, ActivityUpsert,
    ImportMapping, NewActivity,
};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn parse_optional_uuid(value: Option<&str>, field: &str) -> Result<Option<Uuid>> {
    value.map(|raw| parse_uuid(raw, field)).transpose()
}

fn parse_activity_date(value: &str) -> NaiveDate {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc).date_naive())
        .or_else(|_| NaiveDate::parse_from_str(value, "%Y-%m-%d"))
        .unwrap_or_else(|err| {
            log::error!("Failed to parse activity date '{}': {}", value, err);
            Utc::now().date_naive()
        })
}

fn status_to_db(status: Option<&ActivityStatus>) -> String {
    status
        .map(|value| match value {
            ActivityStatus::Posted => "POSTED",
            ActivityStatus::Pending => "PENDING",
            ActivityStatus::Draft => "DRAFT",
            ActivityStatus::Void => "VOID",
        })
        .unwrap_or("POSTED")
        .to_string()
}

fn status_from_db(status: &str) -> ActivityStatus {
    match status {
        "POSTED" => ActivityStatus::Posted,
        "PENDING" => ActivityStatus::Pending,
        "DRAFT" => ActivityStatus::Draft,
        "VOID" => ActivityStatus::Void,
        _ => ActivityStatus::Posted,
    }
}

fn decimal_from_bigdecimal(value: &BigDecimal, field_name: &str) -> Decimal {
    parse_decimal_string_tolerant(&value.to_string(), field_name)
}

fn optional_decimal_from_bigdecimal(
    value: &Option<BigDecimal>,
    field_name: &str,
) -> Option<Decimal> {
    value.as_ref().map(|raw| decimal_from_bigdecimal(raw, field_name))
}

fn bigdecimal_from_decimal(value: &Decimal) -> BigDecimal {
    BigDecimal::from_str(&value.to_string()).unwrap_or_else(|_| BigDecimal::from(0))
}

fn option_bigdecimal_from_decimal(value: Option<Decimal>) -> Option<BigDecimal> {
    value.map(|raw| bigdecimal_from_decimal(&raw))
}

fn optional_json_from_string(value: Option<String>, field: &str) -> Result<Option<Value>> {
    value.map(|raw| {
        serde_json::from_str(&raw).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid {field} JSON: {err}"
            )))
        })
    })
    .transpose()
}

fn rfc3339_from_date(value: NaiveDate) -> String {
    Utc.from_utc_datetime(&value.and_hms_opt(0, 0, 0).unwrap_or_default())
        .to_rfc3339()
}

#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    Debug,
    Clone,
    Default,
)]
#[diesel(table_name = crate::schema::wf_activities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActivityDB {
    pub id: Uuid,
    pub account_id: Uuid,
    pub asset_id: Option<Uuid>,
    pub activity_type: String,
    pub activity_type_override: Option<String>,
    pub source_type: Option<String>,
    pub subtype: Option<String>,
    pub status: String,
    pub activity_date: NaiveDate,
    pub settlement_date: Option<NaiveDate>,
    pub quantity: Option<BigDecimal>,
    pub unit_price: Option<BigDecimal>,
    pub amount: Option<BigDecimal>,
    pub fee: Option<BigDecimal>,
    pub currency: String,
    pub fx_rate: Option<BigDecimal>,
    pub notes: Option<String>,
    pub metadata: Option<Value>,
    pub source_system: Option<String>,
    pub source_record_id: Option<String>,
    pub source_group_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub import_run_id: Option<Uuid>,
    pub is_user_modified: bool,
    pub needs_review: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable, AsChangeset, Queryable, Identifiable)]
#[diesel(primary_key(account_id))]
#[diesel(table_name = crate::schema::wf_activity_import_profiles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ImportMappingDB {
    pub account_id: Uuid,
    pub name: String,
    pub config: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ActivityDB> for Activity {
    fn from(db: ActivityDB) -> Self {
        Self {
            id: db.id.to_string(),
            account_id: db.account_id.to_string(),
            asset_id: db.asset_id.map(|value| value.to_string()),
            activity_type: db.activity_type,
            activity_type_override: db.activity_type_override,
            source_type: db.source_type,
            subtype: db.subtype,
            status: status_from_db(&db.status),
            activity_date: Utc.from_utc_datetime(&db.activity_date.and_hms_opt(0, 0, 0).unwrap_or_default()),
            settlement_date: db
                .settlement_date
                .map(|value| Utc.from_utc_datetime(&value.and_hms_opt(0, 0, 0).unwrap_or_default())),
            quantity: optional_decimal_from_bigdecimal(&db.quantity, "quantity"),
            unit_price: optional_decimal_from_bigdecimal(&db.unit_price, "unit_price"),
            amount: optional_decimal_from_bigdecimal(&db.amount, "amount"),
            fee: optional_decimal_from_bigdecimal(&db.fee, "fee"),
            currency: db.currency,
            fx_rate: optional_decimal_from_bigdecimal(&db.fx_rate, "fx_rate"),
            notes: db.notes,
            metadata: db.metadata,
            source_system: db.source_system,
            source_record_id: db.source_record_id,
            source_group_id: db.source_group_id,
            idempotency_key: db.idempotency_key,
            import_run_id: db.import_run_id.map(|value| value.to_string()),
            is_user_modified: db.is_user_modified,
            needs_review: db.needs_review,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl ActivityDB {
    pub fn from_new_activity(domain: NewActivity) -> Result<Self> {
        let now = Utc::now();
        Ok(Self {
            id: domain
                .id
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(|value| parse_uuid(value, "activity_id"))
                .transpose()?
                .unwrap_or_else(Uuid::nil),
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            asset_id: parse_optional_uuid(domain.get_symbol_id(), "asset_id")?,
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status: status_to_db(domain.status.as_ref()),
            activity_date: parse_activity_date(&domain.activity_date),
            settlement_date: None,
            quantity: option_bigdecimal_from_decimal(domain.quantity),
            unit_price: option_bigdecimal_from_decimal(domain.unit_price),
            amount: option_bigdecimal_from_decimal(domain.amount),
            fee: option_bigdecimal_from_decimal(domain.fee),
            currency: domain.currency,
            fx_rate: option_bigdecimal_from_decimal(domain.fx_rate),
            notes: domain.notes,
            metadata: optional_json_from_string(domain.metadata, "metadata")?,
            source_system: domain.source_system.or(Some("MANUAL".to_string())),
            source_record_id: domain.source_record_id,
            source_group_id: domain.source_group_id,
            idempotency_key: domain.idempotency_key,
            import_run_id: None,
            is_user_modified: false,
            needs_review: domain.needs_review.unwrap_or(false),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn from_activity_update(domain: ActivityUpdate) -> Result<Self> {
        let now = Utc::now();
        Ok(Self {
            id: parse_uuid(&domain.id, "activity_id")?,
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            asset_id: parse_optional_uuid(domain.get_symbol_id(), "asset_id")?,
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status: status_to_db(domain.status.as_ref()),
            activity_date: parse_activity_date(&domain.activity_date),
            settlement_date: None,
            quantity: option_bigdecimal_from_decimal(domain.quantity.flatten()),
            unit_price: option_bigdecimal_from_decimal(domain.unit_price.flatten()),
            amount: option_bigdecimal_from_decimal(domain.amount.flatten()),
            fee: option_bigdecimal_from_decimal(domain.fee.flatten()),
            currency: domain.currency,
            fx_rate: option_bigdecimal_from_decimal(domain.fx_rate.flatten()),
            notes: domain.notes,
            metadata: optional_json_from_string(domain.metadata, "metadata")?,
            source_system: None,
            source_record_id: None,
            source_group_id: None,
            idempotency_key: None,
            import_run_id: None,
            is_user_modified: true,
            needs_review: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn from_activity_upsert(domain: ActivityUpsert) -> Result<Self> {
        let now = Utc::now();
        Ok(Self {
            id: parse_uuid(&domain.id, "activity_id")?,
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            asset_id: parse_optional_uuid(domain.asset_id.as_deref(), "asset_id")?,
            activity_type: domain.activity_type,
            activity_type_override: None,
            source_type: None,
            subtype: domain.subtype,
            status: status_to_db(domain.status.as_ref()),
            activity_date: parse_activity_date(&domain.activity_date),
            settlement_date: None,
            quantity: option_bigdecimal_from_decimal(domain.quantity),
            unit_price: option_bigdecimal_from_decimal(domain.unit_price),
            amount: option_bigdecimal_from_decimal(domain.amount),
            fee: option_bigdecimal_from_decimal(domain.fee),
            currency: domain.currency,
            fx_rate: option_bigdecimal_from_decimal(domain.fx_rate),
            notes: domain.notes,
            metadata: optional_json_from_string(domain.metadata, "metadata")?,
            source_system: domain.source_system,
            source_record_id: domain.source_record_id,
            source_group_id: domain.source_group_id,
            idempotency_key: domain.idempotency_key,
            import_run_id: parse_optional_uuid(domain.import_run_id.as_deref(), "import_run_id")?,
            is_user_modified: false,
            needs_review: domain.needs_review.unwrap_or(false),
            created_at: now,
            updated_at: now,
        })
    }
}

impl From<ImportMappingDB> for ImportMapping {
    fn from(db: ImportMappingDB) -> Self {
        Self {
            account_id: db.account_id.to_string(),
            name: db.name,
            config: db.config.to_string(),
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
        }
    }
}

impl TryFrom<ImportMapping> for ImportMappingDB {
    type Error = Error;

    fn try_from(domain: ImportMapping) -> Result<Self> {
        Ok(Self {
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            name: domain.name,
            config: serde_json::from_str(&domain.config).map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid import mapping config JSON: {err}"
                )))
            })?,
            created_at: Utc.from_utc_datetime(&domain.created_at),
            updated_at: Utc.from_utc_datetime(&domain.updated_at),
        })
    }
}

pub fn apply_decimal_patch(
    existing: Option<BigDecimal>,
    patch: Option<Option<Decimal>>,
) -> Option<BigDecimal> {
    match patch {
        None => existing,
        Some(None) => None,
        Some(Some(value)) => Some(bigdecimal_from_decimal(&value)),
    }
}

pub fn format_activity_date(value: NaiveDate) -> String {
    rfc3339_from_date(value)
}

pub fn format_activity_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}
