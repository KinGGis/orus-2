use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::limits::{ContributionLimit, NewContributionLimit};
use wealthfolio_core::{Error, Result};

#[derive(Queryable, Identifiable, Selectable, PartialEq, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_contribution_limits)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ContributionLimitDB {
    pub id: Uuid,
    pub group_name: String,
    pub contribution_year: i32,
    pub limit_amount: BigDecimal,
    pub account_ids: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_contribution_limits)]
pub struct NewContributionLimitDB {
    pub id: Option<Uuid>,
    pub group_name: String,
    pub contribution_year: i32,
    pub limit_amount: BigDecimal,
    pub account_ids: Option<Value>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

fn bigdecimal_from_f64(value: f64, field: &str) -> Result<BigDecimal> {
    BigDecimal::from_str(&value.to_string()).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} decimal: {err}"
        )))
    })
}

fn f64_from_bigdecimal(value: &BigDecimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or_default()
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn parse_optional_date(value: Option<String>, field: &str) -> Result<Option<NaiveDate>> {
    value
        .map(|date| {
            NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|err| {
                Error::Validation(ValidationError::InvalidInput(format!(
                    "Invalid {field} date '{date}': {err}"
                )))
            })
        })
        .transpose()
}

fn format_optional_date(value: Option<NaiveDate>) -> Option<String> {
    value.map(|date| date.format("%Y-%m-%d").to_string())
}

fn parse_account_ids(value: Option<String>) -> Option<Value> {
    value.and_then(|account_ids| {
        let ids = account_ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();

        if ids.is_empty() {
            None
        } else {
            Some(Value::Array(ids.into_iter().map(Value::String).collect()))
        }
    })
}

fn format_account_ids(value: Option<Value>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let Value::Array(items) = value else {
        return Err(Error::Validation(ValidationError::InvalidInput(
            "Invalid contribution limit account_ids JSON shape".to_string(),
        )));
    };

    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        let Value::String(id) = item else {
            return Err(Error::Validation(ValidationError::InvalidInput(
                "Invalid contribution limit account_ids JSON element".to_string(),
            )));
        };
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            ids.push(trimmed.to_string());
        }
    }

    if ids.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ids.join(",")))
    }
}

fn naive_utc(value: DateTime<Utc>) -> NaiveDateTime {
    value.naive_utc()
}

impl TryFrom<ContributionLimitDB> for ContributionLimit {
    type Error = Error;

    fn try_from(db: ContributionLimitDB) -> Result<Self> {
        Ok(Self {
            id: db.id.to_string(),
            group_name: db.group_name,
            contribution_year: db.contribution_year,
            limit_amount: f64_from_bigdecimal(&db.limit_amount),
            account_ids: format_account_ids(db.account_ids)?,
            created_at: naive_utc(db.created_at),
            updated_at: naive_utc(db.updated_at),
            start_date: format_optional_date(db.start_date),
            end_date: format_optional_date(db.end_date),
        })
    }
}

impl TryFrom<NewContributionLimit> for NewContributionLimitDB {
    type Error = Error;

    fn try_from(domain: NewContributionLimit) -> Result<Self> {
        Ok(Self {
            id: domain
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "contribution_limit_id"))
                .transpose()?,
            group_name: domain.group_name,
            contribution_year: domain.contribution_year,
            limit_amount: bigdecimal_from_f64(domain.limit_amount, "limit_amount")?,
            account_ids: parse_account_ids(domain.account_ids),
            start_date: parse_optional_date(domain.start_date, "start_date")?,
            end_date: parse_optional_date(domain.end_date, "end_date")?,
        })
    }
}