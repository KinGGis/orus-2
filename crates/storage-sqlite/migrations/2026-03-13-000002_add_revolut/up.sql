-- Revolut Business Integration Migration
-- Tables for OAuth tokens, account sync, and transaction history

-- ============================================================================
-- REVOLUT_AUTH_TOKENS TABLE
-- ============================================================================
-- Secure storage for OAuth 2.0 tokens. Tokens should be encrypted at app layer.
-- Only one active token set per organization (DFC single-tenant).

CREATE TABLE revolut_auth_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    token_type TEXT NOT NULL DEFAULT 'Bearer',
    scope TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_revolut_auth_tokens_expires ON revolut_auth_tokens(expires_at);

-- ============================================================================
-- REVOLUT_ACCOUNTS TABLE
-- ============================================================================
-- Cached Revolut Business account data. Maps to WF accounts (type=CASH).

CREATE TABLE revolut_accounts (
    id TEXT PRIMARY KEY NOT NULL,
    revolut_account_id TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    currency TEXT NOT NULL,
    balance TEXT NOT NULL DEFAULT '0',
    state TEXT NOT NULL DEFAULT 'active',
    wf_account_id TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CONSTRAINT fk_revolut_accounts_wf_account FOREIGN KEY (wf_account_id)
        REFERENCES accounts(id) ON DELETE SET NULL ON UPDATE CASCADE,

    CHECK (state IN ('active', 'inactive', 'blocked'))
);

CREATE INDEX idx_revolut_accounts_revolut_id ON revolut_accounts(revolut_account_id);
CREATE INDEX idx_revolut_accounts_currency ON revolut_accounts(currency);
CREATE INDEX idx_revolut_accounts_wf_account ON revolut_accounts(wf_account_id);

-- ============================================================================
-- REVOLUT_TRANSACTIONS TABLE
-- ============================================================================
-- Transaction history from Revolut. Maps to WF activities.

CREATE TABLE revolut_transactions (
    id TEXT PRIMARY KEY NOT NULL,
    revolut_transaction_id TEXT UNIQUE NOT NULL,
    revolut_account_id TEXT NOT NULL,
    type TEXT NOT NULL,
    state TEXT NOT NULL,
    amount TEXT NOT NULL,
    currency TEXT NOT NULL,
    description TEXT,
    merchant_name TEXT,
    merchant_category TEXT,
    reference TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    balance_after TEXT,
    wf_activity_id TEXT,
    synced_to_activities INTEGER NOT NULL DEFAULT 0,

    CONSTRAINT fk_revolut_tx_account FOREIGN KEY (revolut_account_id)
        REFERENCES revolut_accounts(revolut_account_id) ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_revolut_tx_wf_activity FOREIGN KEY (wf_activity_id)
        REFERENCES activities(id) ON DELETE SET NULL ON UPDATE CASCADE,

    CHECK (synced_to_activities IN (0, 1)),
    CHECK (state IN ('pending', 'completed', 'declined', 'failed', 'reverted'))
);

CREATE INDEX idx_revolut_tx_revolut_id ON revolut_transactions(revolut_transaction_id);
CREATE INDEX idx_revolut_tx_account_id ON revolut_transactions(revolut_account_id);
CREATE INDEX idx_revolut_tx_synced ON revolut_transactions(synced_to_activities);
CREATE INDEX idx_revolut_tx_completed ON revolut_transactions(completed_at);

-- ============================================================================
-- REVOLUT_SYNC_LOG TABLE
-- ============================================================================
-- Audit log for sync operations.

CREATE TABLE revolut_sync_log (
    id TEXT PRIMARY KEY NOT NULL,
    sync_type TEXT NOT NULL,
    status TEXT NOT NULL,
    accounts_synced INTEGER DEFAULT 0,
    transactions_synced INTEGER DEFAULT 0,
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT,

    CHECK (sync_type IN ('full', 'incremental', 'accounts_only', 'transactions_only')),
    CHECK (status IN ('started', 'success', 'partial', 'error'))
);

CREATE INDEX idx_revolut_sync_log_status ON revolut_sync_log(status, started_at DESC);
CREATE INDEX idx_revolut_sync_log_type ON revolut_sync_log(sync_type, started_at DESC);
