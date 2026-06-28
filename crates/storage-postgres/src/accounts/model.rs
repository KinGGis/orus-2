use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use wealthfolio_core::accounts::{Account, AccountUpdate, NewAccount, TrackingMode};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountDB {
    pub id: Uuid,
    pub name: String,
    pub account_type: String,
    #[diesel(column_name = group_)]
    pub group: Option<String>,
    pub currency: String,
    pub is_default: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub platform_id: Option<String>,
    pub account_number: Option<String>,
    pub meta: Option<Value>,
    pub provider: Option<String>,
    pub provider_account_id: Option<String>,
    pub is_archived: bool,
    pub tracking_mode: String,
}

fn tracking_mode_to_db(value: TrackingMode) -> String {
    match value {
        TrackingMode::Transactions => "TRANSACTIONS",
        TrackingMode::Holdings => "HOLDINGS",
        TrackingMode::NotSet => "NOT_SET",
    }
    .to_string()
}

fn tracking_mode_from_db(value: &str) -> TrackingMode {
    match value {
        "TRANSACTIONS" => TrackingMode::Transactions,
        "HOLDINGS" => TrackingMode::Holdings,
        _ => TrackingMode::NotSet,
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn parse_optional_platform_id(value: &Option<String>, field: &str) -> Result<Option<String>> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            if raw.len() > 255 {
                Err(Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid {field}: value too long"
                ))))
            } else {
                Ok(raw.to_string())
            }
        })
        .transpose()
}

fn parse_optional_json(value: &Option<String>, field: &str) -> Result<Option<Value>> {
    value.as_deref()
        .map(|raw| {
            serde_json::from_str(raw).map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid {field} JSON: {err}"
                )))
            })
        })
        .transpose()
}

impl From<AccountDB> for Account {
    fn from(db: AccountDB) -> Self {
        Self {
            id: db.id.to_string(),
            name: db.name,
            account_type: db.account_type,
            group: db.group,
            currency: db.currency,
            is_default: db.is_default,
            is_active: db.is_active,
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
            platform_id: db.platform_id,
            account_number: db.account_number,
            meta: db.meta.map(|value| value.to_string()),
            provider: db.provider,
            provider_account_id: db.provider_account_id,
            is_archived: db.is_archived,
            tracking_mode: tracking_mode_from_db(&db.tracking_mode),
        }
    }
}

impl AccountDB {
    pub fn from_new_account(domain: NewAccount) -> Result<Self> {
        let now = Utc::now();
        Ok(Self {
            id: match domain.id.as_deref() {
                Some(id) if !id.is_empty() => parse_uuid(id, "account_id")?,
                _ => Uuid::new_v4(),
            },
            name: domain.name,
            account_type: domain.account_type,
            group: domain.group,
            currency: domain.currency,
            is_default: domain.is_default,
            is_active: domain.is_active,
            created_at: now,
            updated_at: now,
            platform_id: parse_optional_platform_id(&domain.platform_id, "platform_id")?,
            account_number: domain.account_number,
            meta: parse_optional_json(&domain.meta, "meta")?,
            provider: domain.provider,
            provider_account_id: domain.provider_account_id,
            is_archived: domain.is_archived,
            tracking_mode: tracking_mode_to_db(domain.tracking_mode),
        })
    }

    pub fn from_account_update(domain: AccountUpdate) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(
                domain.id.as_deref().unwrap_or_default(),
                "account_id",
            )?,
            name: domain.name,
            account_type: domain.account_type,
            group: domain.group,
            currency: String::new(),
            is_default: domain.is_default,
            is_active: domain.is_active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            platform_id: parse_optional_platform_id(&domain.platform_id, "platform_id")?,
            account_number: domain.account_number,
            meta: parse_optional_json(&domain.meta, "meta")?,
            provider: domain.provider,
            provider_account_id: domain.provider_account_id,
            is_archived: domain.is_archived.unwrap_or(false),
            tracking_mode: tracking_mode_to_db(domain.tracking_mode.unwrap_or(TrackingMode::NotSet)),
        })
    }
}