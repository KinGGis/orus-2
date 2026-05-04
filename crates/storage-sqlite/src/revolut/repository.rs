//! Repository implementation for Revolut integration.

use diesel::prelude::*;
use diesel::r2d2::{self, Pool};
use diesel::sqlite::SqliteConnection;
use std::sync::Arc;

use crate::db::get_connection;
use crate::errors::StorageError;
use crate::schema::{revolut_accounts, revolut_auth_tokens, revolut_sync_log, revolut_transactions};

use super::model::{RevolutAccountDB, RevolutAuthTokenDB, RevolutSyncLogDB, RevolutTransactionDB};
use wealthfolio_core::errors::Result;
use wealthfolio_core::revolut::{
    NewRevolutAuthToken, RevolutAccount, RevolutAuthToken, RevolutSyncLog, RevolutTransaction,
};

/// Repository for managing Revolut data in the database
pub struct RevolutRepository {
    pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>,
}

impl RevolutRepository {
    /// Creates a new RevolutRepository instance
    pub fn new(pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>) -> Self {
        Self { pool }
    }

    // ========== Auth Tokens ==========

    /// Saves a new authentication token
    pub fn save_token(&self, new_token: NewRevolutAuthToken) -> Result<RevolutAuthToken> {
        let mut conn = get_connection(&self.pool)?;

        let mut token_db: RevolutAuthTokenDB = new_token.into();
        token_db.id = uuid::Uuid::new_v4().to_string();

        diesel::insert_into(revolut_auth_tokens::table)
            .values(&token_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(token_db.into())
    }

    /// Gets the most recent valid token (not expired)
    pub fn get_active_token(&self) -> Result<Option<RevolutAuthToken>> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let result = revolut_auth_tokens::table
            .select(RevolutAuthTokenDB::as_select())
            .filter(revolut_auth_tokens::expires_at.gt(&now))
            .order(revolut_auth_tokens::created_at.desc())
            .first::<RevolutAuthTokenDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    /// Deletes all tokens (used when rotating credentials)
    pub fn delete_all_tokens(&self) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(revolut_auth_tokens::table)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    // ========== Accounts ==========

    /// Upserts a Revolut account (insert or update on conflict)
    pub fn upsert_account(&self, account: RevolutAccount) -> Result<RevolutAccount> {
        let mut conn = get_connection(&self.pool)?;

        let account_db: RevolutAccountDB = account.into();

        diesel::insert_into(revolut_accounts::table)
            .values(&account_db)
            .on_conflict(revolut_accounts::revolut_account_id)
            .do_update()
            .set((
                revolut_accounts::name.eq(&account_db.name),
                revolut_accounts::balance.eq(&account_db.balance),
                revolut_accounts::state.eq(&account_db.state),
                revolut_accounts::last_synced_at.eq(&account_db.last_synced_at),
                revolut_accounts::updated_at.eq(&account_db.updated_at),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(account_db.into())
    }

    /// Lists all Revolut accounts
    pub fn list_accounts(&self) -> Result<Vec<RevolutAccount>> {
        let mut conn = get_connection(&self.pool)?;

        let results = revolut_accounts::table
            .select(RevolutAccountDB::as_select())
            .order(revolut_accounts::name.asc())
            .load::<RevolutAccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Gets a Revolut account by its Revolut ID
    pub fn get_account_by_revolut_id(&self, revolut_account_id: &str) -> Result<Option<RevolutAccount>> {
        let mut conn = get_connection(&self.pool)?;

        let result = revolut_accounts::table
            .select(RevolutAccountDB::as_select())
            .filter(revolut_accounts::revolut_account_id.eq(revolut_account_id))
            .first::<RevolutAccountDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    /// Links a Revolut account to a Wealthfolio account
    pub fn link_to_wf_account(&self, revolut_account_id: &str, wf_account_id: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        diesel::update(
            revolut_accounts::table.filter(revolut_accounts::revolut_account_id.eq(revolut_account_id)),
        )
        .set((
            revolut_accounts::wf_account_id.eq(Some(wf_account_id)),
            revolut_accounts::updated_at.eq(&now),
        ))
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        Ok(())
    }

    // ========== Transactions ==========

    /// Upserts a Revolut transaction
    pub fn upsert_transaction(&self, tx: RevolutTransaction) -> Result<RevolutTransaction> {
        let mut conn = get_connection(&self.pool)?;

        let tx_db: RevolutTransactionDB = tx.into();

        diesel::insert_into(revolut_transactions::table)
            .values(&tx_db)
            .on_conflict(revolut_transactions::revolut_transaction_id)
            .do_update()
            .set((
                revolut_transactions::state.eq(&tx_db.state),
                revolut_transactions::completed_at.eq(&tx_db.completed_at),
                revolut_transactions::synced_to_activities.eq(&tx_db.synced_to_activities),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(tx_db.into())
    }

    /// Lists transactions for a specific Revolut account
    pub fn list_transactions_for_account(&self, revolut_account_id: &str) -> Result<Vec<RevolutTransaction>> {
        let mut conn = get_connection(&self.pool)?;

        let results = revolut_transactions::table
            .select(RevolutTransactionDB::as_select())
            .filter(revolut_transactions::revolut_account_id.eq(revolut_account_id))
            .order(revolut_transactions::created_at.desc())
            .load::<RevolutTransactionDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Lists unsynced transactions (not yet imported to WF activities)
    pub fn list_unsynced_transactions(&self) -> Result<Vec<RevolutTransaction>> {
        let mut conn = get_connection(&self.pool)?;

        let results = revolut_transactions::table
            .select(RevolutTransactionDB::as_select())
            .filter(revolut_transactions::synced_to_activities.eq(0))
            .order(revolut_transactions::created_at.asc())
            .load::<RevolutTransactionDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Links a Revolut transaction to a Wealthfolio activity
    pub fn link_to_wf_activity(&self, revolut_tx_id: &str, wf_activity_id: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        diesel::update(
            revolut_transactions::table.filter(revolut_transactions::revolut_transaction_id.eq(revolut_tx_id)),
        )
        .set((
            revolut_transactions::wf_activity_id.eq(Some(wf_activity_id)),
            revolut_transactions::synced_to_activities.eq(1),
        ))
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        Ok(())
    }

    // ========== Sync Log ==========

    /// Creates a new sync log entry
    pub fn create_sync_log(&self, sync_type: &str) -> Result<RevolutSyncLog> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let log = RevolutSyncLogDB {
            id: uuid::Uuid::new_v4().to_string(),
            sync_type: sync_type.to_string(),
            status: "started".to_string(),
            accounts_synced: None,
            transactions_synced: None,
            error_message: None,
            started_at: now,
            completed_at: None,
        };

        diesel::insert_into(revolut_sync_log::table)
            .values(&log)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(log.into())
    }

    /// Completes a sync log entry with success
    pub fn complete_sync_log(&self, log_id: &str, accounts_synced: i32, transactions_synced: i32) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        diesel::update(revolut_sync_log::table.find(log_id))
            .set((
                revolut_sync_log::status.eq("success"),
                revolut_sync_log::completed_at.eq(Some(&now)),
                revolut_sync_log::accounts_synced.eq(Some(accounts_synced)),
                revolut_sync_log::transactions_synced.eq(Some(transactions_synced)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    /// Fails a sync log entry with an error message
    pub fn fail_sync_log(&self, log_id: &str, error: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        diesel::update(revolut_sync_log::table.find(log_id))
            .set((
                revolut_sync_log::status.eq("error"),
                revolut_sync_log::completed_at.eq(Some(&now)),
                revolut_sync_log::error_message.eq(Some(error)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    /// Gets the latest sync log entries
    pub fn get_recent_sync_logs(&self, limit: i64) -> Result<Vec<RevolutSyncLog>> {
        let mut conn = get_connection(&self.pool)?;

        let results = revolut_sync_log::table
            .select(RevolutSyncLogDB::as_select())
            .order(revolut_sync_log::started_at.desc())
            .limit(limit)
            .load::<RevolutSyncLogDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}
