use async_trait::async_trait;
use wealthfolio_core::revolut::{
    NewRevolutAuthToken, RevolutAccount, RevolutAuthToken, RevolutSyncLog, RevolutTransaction,
};

#[async_trait]
pub trait RevolutStore: Send + Sync {
    fn save_token(
        &self,
        new_token: NewRevolutAuthToken,
    ) -> wealthfolio_core::Result<RevolutAuthToken>;
    fn get_active_token(&self) -> wealthfolio_core::Result<Option<RevolutAuthToken>>;
    fn delete_all_tokens(&self) -> wealthfolio_core::Result<()>;
    fn upsert_account(&self, account: RevolutAccount) -> wealthfolio_core::Result<RevolutAccount>;
    fn list_accounts(&self) -> wealthfolio_core::Result<Vec<RevolutAccount>>;
    fn upsert_transaction(
        &self,
        tx: RevolutTransaction,
    ) -> wealthfolio_core::Result<RevolutTransaction>;
    fn list_unsynced_transactions(&self) -> wealthfolio_core::Result<Vec<RevolutTransaction>>;
    fn create_sync_log(&self, sync_type: &str) -> wealthfolio_core::Result<RevolutSyncLog>;
    fn complete_sync_log(
        &self,
        log_id: &str,
        accounts_synced: i32,
        transactions_synced: i32,
    ) -> wealthfolio_core::Result<()>;
    fn fail_sync_log(&self, log_id: &str, error: &str) -> wealthfolio_core::Result<()>;
    fn get_recent_sync_logs(&self, limit: i64) -> wealthfolio_core::Result<Vec<RevolutSyncLog>>;
}

#[async_trait]
impl RevolutStore for wealthfolio_storage_sqlite::revolut::RevolutRepository {
    fn save_token(
        &self,
        new_token: NewRevolutAuthToken,
    ) -> wealthfolio_core::Result<RevolutAuthToken> {
        self.save_token(new_token)
    }

    fn get_active_token(&self) -> wealthfolio_core::Result<Option<RevolutAuthToken>> {
        self.get_active_token()
    }

    fn delete_all_tokens(&self) -> wealthfolio_core::Result<()> {
        self.delete_all_tokens()
    }

    fn upsert_account(&self, account: RevolutAccount) -> wealthfolio_core::Result<RevolutAccount> {
        self.upsert_account(account)
    }

    fn list_accounts(&self) -> wealthfolio_core::Result<Vec<RevolutAccount>> {
        self.list_accounts()
    }

    fn upsert_transaction(
        &self,
        tx: RevolutTransaction,
    ) -> wealthfolio_core::Result<RevolutTransaction> {
        self.upsert_transaction(tx)
    }

    fn list_unsynced_transactions(&self) -> wealthfolio_core::Result<Vec<RevolutTransaction>> {
        self.list_unsynced_transactions()
    }

    fn create_sync_log(&self, sync_type: &str) -> wealthfolio_core::Result<RevolutSyncLog> {
        self.create_sync_log(sync_type)
    }

    fn complete_sync_log(
        &self,
        log_id: &str,
        accounts_synced: i32,
        transactions_synced: i32,
    ) -> wealthfolio_core::Result<()> {
        self.complete_sync_log(log_id, accounts_synced, transactions_synced)
    }

    fn fail_sync_log(&self, log_id: &str, error: &str) -> wealthfolio_core::Result<()> {
        self.fail_sync_log(log_id, error)
    }

    fn get_recent_sync_logs(&self, limit: i64) -> wealthfolio_core::Result<Vec<RevolutSyncLog>> {
        self.get_recent_sync_logs(limit)
    }
}

#[async_trait]
impl RevolutStore for wealthfolio_storage_postgres::revolut::RevolutRepository {
    fn save_token(
        &self,
        new_token: NewRevolutAuthToken,
    ) -> wealthfolio_core::Result<RevolutAuthToken> {
        self.save_token(new_token)
    }

    fn get_active_token(&self) -> wealthfolio_core::Result<Option<RevolutAuthToken>> {
        self.get_active_token()
    }

    fn delete_all_tokens(&self) -> wealthfolio_core::Result<()> {
        self.delete_all_tokens()
    }

    fn upsert_account(&self, account: RevolutAccount) -> wealthfolio_core::Result<RevolutAccount> {
        self.upsert_account(account)
    }

    fn list_accounts(&self) -> wealthfolio_core::Result<Vec<RevolutAccount>> {
        self.list_accounts()
    }

    fn upsert_transaction(
        &self,
        tx: RevolutTransaction,
    ) -> wealthfolio_core::Result<RevolutTransaction> {
        self.upsert_transaction(tx)
    }

    fn list_unsynced_transactions(&self) -> wealthfolio_core::Result<Vec<RevolutTransaction>> {
        self.list_unsynced_transactions()
    }

    fn create_sync_log(&self, sync_type: &str) -> wealthfolio_core::Result<RevolutSyncLog> {
        self.create_sync_log(sync_type)
    }

    fn complete_sync_log(
        &self,
        log_id: &str,
        accounts_synced: i32,
        transactions_synced: i32,
    ) -> wealthfolio_core::Result<()> {
        self.complete_sync_log(log_id, accounts_synced, transactions_synced)
    }

    fn fail_sync_log(&self, log_id: &str, error: &str) -> wealthfolio_core::Result<()> {
        self.fail_sync_log(log_id, error)
    }

    fn get_recent_sync_logs(&self, limit: i64) -> wealthfolio_core::Result<Vec<RevolutSyncLog>> {
        self.get_recent_sync_logs(limit)
    }
}