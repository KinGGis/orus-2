use std::str::FromStr;
use std::sync::Arc;

use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{
    wf_revolut_accounts, wf_revolut_auth_tokens, wf_revolut_sync_log, wf_revolut_transactions,
};
use wealthfolio_core::errors::{Result, ValidationError};
use wealthfolio_core::revolut::{
    NewRevolutAuthToken, RevolutAccount, RevolutAuthToken, RevolutSyncLog, RevolutTransaction,
};
use wealthfolio_core::Error;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_revolut_auth_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct RevolutAuthTokenDB {
    id: Uuid,
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
    scope: Option<String>,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_revolut_accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct RevolutAccountDB {
    id: Uuid,
    revolut_account_id: String,
    name: String,
    currency: String,
    balance: BigDecimal,
    state: String,
    wf_account_id: Option<Uuid>,
    last_synced_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_revolut_transactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct RevolutTransactionDB {
    id: Uuid,
    revolut_transaction_id: String,
    revolut_account_id: String,
    #[diesel(column_name = tx_type)]
    tx_type: String,
    state: String,
    amount: BigDecimal,
    currency: String,
    description: Option<String>,
    merchant_name: Option<String>,
    merchant_category: Option<String>,
    reference: Option<String>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    balance_after: Option<BigDecimal>,
    wf_activity_id: Option<Uuid>,
    synced_to_activities: bool,
}

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_revolut_sync_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct RevolutSyncLogDB {
    id: Uuid,
    sync_type: String,
    status: String,
    accounts_synced: Option<i32>,
    transactions_synced: Option<i32>,
    error_message: Option<String>,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn parse_optional_uuid(value: &Option<String>, field: &str) -> Result<Option<Uuid>> {
    value
        .as_deref()
        .map(|raw| parse_uuid(raw, field))
        .transpose()
}

fn parse_rfc3339(value: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid {field} timestamp: {err}"
            )))
        })
}

fn parse_optional_rfc3339(value: &Option<String>, field: &str) -> Result<Option<DateTime<Utc>>> {
    value
        .as_deref()
        .map(|raw| parse_rfc3339(raw, field))
        .transpose()
}

fn parse_decimal(value: &str, field: &str) -> Result<BigDecimal> {
    BigDecimal::from_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} decimal: {err}"
        )))
    })
}

impl From<RevolutAuthTokenDB> for RevolutAuthToken {
    fn from(db: RevolutAuthTokenDB) -> Self {
        Self {
            id: db.id.to_string(),
            access_token: db.access_token,
            refresh_token: db.refresh_token,
            token_type: db.token_type,
            scope: db.scope,
            expires_at: db.expires_at.to_rfc3339(),
            created_at: db.created_at.to_rfc3339(),
            updated_at: db.updated_at.to_rfc3339(),
        }
    }
}

impl TryFrom<RevolutAccount> for RevolutAccountDB {
    type Error = wealthfolio_core::Error;

    fn try_from(domain: RevolutAccount) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&domain.id, "revolut_account_row_id")?,
            revolut_account_id: domain.revolut_account_id,
            name: domain.name,
            currency: domain.currency,
            balance: parse_decimal(&domain.balance, "balance")?,
            state: domain.state,
            wf_account_id: parse_optional_uuid(&domain.wf_account_id, "wf_account_id")?,
            last_synced_at: parse_optional_rfc3339(&domain.last_synced_at, "last_synced_at")?,
            created_at: parse_rfc3339(&domain.created_at, "created_at")?,
            updated_at: parse_rfc3339(&domain.updated_at, "updated_at")?,
        })
    }
}

impl From<RevolutAccountDB> for RevolutAccount {
    fn from(db: RevolutAccountDB) -> Self {
        Self {
            id: db.id.to_string(),
            revolut_account_id: db.revolut_account_id,
            name: db.name,
            currency: db.currency,
            balance: db.balance.to_string(),
            state: db.state,
            wf_account_id: db.wf_account_id.map(|value| value.to_string()),
            last_synced_at: db.last_synced_at.map(|value| value.to_rfc3339()),
            created_at: db.created_at.to_rfc3339(),
            updated_at: db.updated_at.to_rfc3339(),
        }
    }
}

impl TryFrom<RevolutTransaction> for RevolutTransactionDB {
    type Error = wealthfolio_core::Error;

    fn try_from(domain: RevolutTransaction) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&domain.id, "revolut_transaction_row_id")?,
            revolut_transaction_id: domain.revolut_transaction_id,
            revolut_account_id: domain.revolut_account_id,
            tx_type: domain.r#type,
            state: domain.state,
            amount: parse_decimal(&domain.amount, "amount")?,
            currency: domain.currency,
            description: domain.description,
            merchant_name: domain.merchant_name,
            merchant_category: domain.merchant_category,
            reference: domain.reference,
            completed_at: parse_optional_rfc3339(&domain.completed_at, "completed_at")?,
            created_at: parse_rfc3339(&domain.created_at, "created_at")?,
            balance_after: domain
                .balance_after
                .as_deref()
                .map(|value| parse_decimal(value, "balance_after"))
                .transpose()?,
            wf_activity_id: parse_optional_uuid(&domain.wf_activity_id, "wf_activity_id")?,
            synced_to_activities: domain.synced_to_activities,
        })
    }
}

impl From<RevolutTransactionDB> for RevolutTransaction {
    fn from(db: RevolutTransactionDB) -> Self {
        Self {
            id: db.id.to_string(),
            revolut_transaction_id: db.revolut_transaction_id,
            revolut_account_id: db.revolut_account_id,
            r#type: db.tx_type,
            state: db.state,
            amount: db.amount.to_string(),
            currency: db.currency,
            description: db.description,
            merchant_name: db.merchant_name,
            merchant_category: db.merchant_category,
            reference: db.reference,
            completed_at: db.completed_at.map(|value| value.to_rfc3339()),
            created_at: db.created_at.to_rfc3339(),
            balance_after: db.balance_after.map(|value| value.to_string()),
            wf_activity_id: db.wf_activity_id.map(|value| value.to_string()),
            synced_to_activities: db.synced_to_activities,
        }
    }
}

impl From<RevolutSyncLogDB> for RevolutSyncLog {
    fn from(db: RevolutSyncLogDB) -> Self {
        Self {
            id: db.id.to_string(),
            sync_type: db.sync_type,
            status: db.status,
            accounts_synced: db.accounts_synced,
            transactions_synced: db.transactions_synced,
            error_message: db.error_message,
            started_at: db.started_at.to_rfc3339(),
            completed_at: db.completed_at.map(|value| value.to_rfc3339()),
        }
    }
}

pub struct RevolutRepository {
    pool: Arc<DbPool>,
}

impl RevolutRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    pub fn save_token(&self, new_token: NewRevolutAuthToken) -> Result<RevolutAuthToken> {
        let now = Utc::now();
        let expires_at = if let Some(value) = new_token.expires_at.as_deref() {
            parse_rfc3339(value, "expires_at")?
        } else if let Some(expires_in) = new_token.expires_in {
            now + Duration::seconds(expires_in)
        } else {
            now + Duration::hours(1)
        };

        let token_db = RevolutAuthTokenDB {
            id: new_token
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "revolut_auth_token_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            access_token: new_token.access_token,
            refresh_token: new_token.refresh_token,
            token_type: new_token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            scope: new_token.scope,
            expires_at,
            created_at: now,
            updated_at: now,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_revolut_auth_tokens::table)
            .values(&token_db)
            .get_result::<RevolutAuthTokenDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn get_active_token(&self) -> Result<Option<RevolutAuthToken>> {
        let mut conn = get_connection(&self.pool)?;
        let result = wf_revolut_auth_tokens::table
            .select(RevolutAuthTokenDB::as_select())
            .filter(wf_revolut_auth_tokens::expires_at.gt(Utc::now()))
            .order(wf_revolut_auth_tokens::created_at.desc())
            .first::<RevolutAuthTokenDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    pub fn delete_all_tokens(&self) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(wf_revolut_auth_tokens::table)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub fn upsert_account(&self, account: RevolutAccount) -> Result<RevolutAccount> {
        let account_db = RevolutAccountDB::try_from(account)?;
        let mut conn = get_connection(&self.pool)?;

        let result = diesel::insert_into(wf_revolut_accounts::table)
            .values(&account_db)
            .on_conflict(wf_revolut_accounts::revolut_account_id)
            .do_update()
            .set((
                wf_revolut_accounts::name.eq(&account_db.name),
                wf_revolut_accounts::balance.eq(&account_db.balance),
                wf_revolut_accounts::state.eq(&account_db.state),
                wf_revolut_accounts::wf_account_id.eq(&account_db.wf_account_id),
                wf_revolut_accounts::last_synced_at.eq(&account_db.last_synced_at),
                wf_revolut_accounts::updated_at.eq(&account_db.updated_at),
            ))
            .get_result::<RevolutAccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn list_accounts(&self) -> Result<Vec<RevolutAccount>> {
        let mut conn = get_connection(&self.pool)?;
        let results = wf_revolut_accounts::table
            .select(RevolutAccountDB::as_select())
            .order(wf_revolut_accounts::name.asc())
            .load::<RevolutAccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    pub fn upsert_transaction(&self, tx: RevolutTransaction) -> Result<RevolutTransaction> {
        let tx_db = RevolutTransactionDB::try_from(tx)?;
        let mut conn = get_connection(&self.pool)?;

        let result = diesel::insert_into(wf_revolut_transactions::table)
            .values(&tx_db)
            .on_conflict(wf_revolut_transactions::revolut_transaction_id)
            .do_update()
            .set((
                wf_revolut_transactions::state.eq(&tx_db.state),
                wf_revolut_transactions::completed_at.eq(&tx_db.completed_at),
                wf_revolut_transactions::balance_after.eq(&tx_db.balance_after),
                wf_revolut_transactions::wf_activity_id.eq(&tx_db.wf_activity_id),
                wf_revolut_transactions::synced_to_activities.eq(&tx_db.synced_to_activities),
            ))
            .get_result::<RevolutTransactionDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn list_unsynced_transactions(&self) -> Result<Vec<RevolutTransaction>> {
        let mut conn = get_connection(&self.pool)?;
        let results = wf_revolut_transactions::table
            .select(RevolutTransactionDB::as_select())
            .filter(wf_revolut_transactions::synced_to_activities.eq(false))
            .order(wf_revolut_transactions::created_at.asc())
            .load::<RevolutTransactionDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    pub fn create_sync_log(&self, sync_type: &str) -> Result<RevolutSyncLog> {
        let log = RevolutSyncLogDB {
            id: Uuid::new_v4(),
            sync_type: sync_type.to_string(),
            status: "started".to_string(),
            accounts_synced: None,
            transactions_synced: None,
            error_message: None,
            started_at: Utc::now(),
            completed_at: None,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_revolut_sync_log::table)
            .values(&log)
            .get_result::<RevolutSyncLogDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn complete_sync_log(
        &self,
        log_id: &str,
        accounts_synced: i32,
        transactions_synced: i32,
    ) -> Result<()> {
        let parsed_log_id = parse_uuid(log_id, "sync_log_id")?;
        let mut conn = get_connection(&self.pool)?;

        diesel::update(wf_revolut_sync_log::table.find(parsed_log_id))
            .set((
                wf_revolut_sync_log::status.eq("success"),
                wf_revolut_sync_log::completed_at.eq(Some(Utc::now())),
                wf_revolut_sync_log::accounts_synced.eq(Some(accounts_synced)),
                wf_revolut_sync_log::transactions_synced.eq(Some(transactions_synced)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub fn fail_sync_log(&self, log_id: &str, error: &str) -> Result<()> {
        let parsed_log_id = parse_uuid(log_id, "sync_log_id")?;
        let mut conn = get_connection(&self.pool)?;

        diesel::update(wf_revolut_sync_log::table.find(parsed_log_id))
            .set((
                wf_revolut_sync_log::status.eq("error"),
                wf_revolut_sync_log::completed_at.eq(Some(Utc::now())),
                wf_revolut_sync_log::error_message.eq(Some(error.to_string())),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub fn get_recent_sync_logs(&self, limit: i64) -> Result<Vec<RevolutSyncLog>> {
        let mut conn = get_connection(&self.pool)?;
        let results = wf_revolut_sync_log::table
            .select(RevolutSyncLogDB::as_select())
            .order(wf_revolut_sync_log::started_at.desc())
            .limit(limit)
            .load::<RevolutSyncLogDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}