-- Revolut Integration Migration Rollback
-- Drop tables in reverse FK order

DROP INDEX IF EXISTS idx_revolut_sync_log_type;
DROP INDEX IF EXISTS idx_revolut_sync_log_status;
DROP TABLE IF EXISTS revolut_sync_log;

DROP INDEX IF EXISTS idx_revolut_tx_completed;
DROP INDEX IF EXISTS idx_revolut_tx_synced;
DROP INDEX IF EXISTS idx_revolut_tx_account_id;
DROP INDEX IF EXISTS idx_revolut_tx_revolut_id;
DROP TABLE IF EXISTS revolut_transactions;

DROP INDEX IF EXISTS idx_revolut_accounts_wf_account;
DROP INDEX IF EXISTS idx_revolut_accounts_currency;
DROP INDEX IF EXISTS idx_revolut_accounts_revolut_id;
DROP TABLE IF EXISTS revolut_accounts;

DROP INDEX IF EXISTS idx_revolut_auth_tokens_expires;
DROP TABLE IF EXISTS revolut_auth_tokens;
