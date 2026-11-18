-- Baseline Wealthfolio schema (wf_* tables), as it stood before migration 0001.
--
-- These tables were first created outside this repository, by the Supabase
-- migrations 20250124000000_wealthfolio_tables and 20260517000000_wf_platform_text_ids
-- (the latter folded in here: platform ids are text slugs). Diesel had already
-- recorded this migration as a placeholder on existing databases, so it never
-- runs there; it only lets a blank Postgres replay 0000..N and reach the same
-- schema, e.g. for integration tests.
--
-- Supabase-only objects are deliberately left out: the "Admin access" RLS
-- policies, wf_is_admin() and the profile sync trigger depend on the auth
-- schema and on tables owned by another system. Row level security is still
-- enabled, which without policies only lets the table owner through.
CREATE TABLE IF NOT EXISTS wf_platforms (
    id TEXT PRIMARY KEY,
    name TEXT,
    url TEXT NOT NULL,
    external_id TEXT,
    kind TEXT NOT NULL DEFAULT 'BROKER',
    website_url TEXT,
    logo_url TEXT
);
-- Accounts (brokerage accounts)
CREATE TABLE IF NOT EXISTS wf_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    "group" TEXT,
    currency TEXT NOT NULL DEFAULT 'EUR',
    is_default BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    platform_id TEXT REFERENCES wf_platforms(id),
    account_number TEXT,
    meta JSONB,
    provider TEXT,
    provider_account_id TEXT,
    is_archived BOOLEAN NOT NULL DEFAULT false,
    tracking_mode TEXT NOT NULL DEFAULT 'AUTOMATIC'
);
CREATE INDEX IF NOT EXISTS idx_wf_accounts_platform ON wf_accounts(platform_id);
CREATE INDEX IF NOT EXISTS idx_wf_accounts_active ON wf_accounts(is_active);
-- Assets (instruments, securities)
CREATE TABLE IF NOT EXISTS wf_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    name TEXT,
    display_code TEXT,
    notes TEXT,
    metadata JSONB,
    is_active BOOLEAN NOT NULL DEFAULT true,
    quote_mode TEXT NOT NULL DEFAULT 'AUTO',
    quote_ccy TEXT NOT NULL DEFAULT 'USD',
    instrument_type TEXT,
    instrument_symbol TEXT,
    instrument_exchange_mic TEXT,
    instrument_key TEXT,
    provider_config JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_assets_symbol ON wf_assets(instrument_symbol);
CREATE INDEX IF NOT EXISTS idx_wf_assets_key ON wf_assets(instrument_key);
-- Activities (transactions, trades, dividends, etc.)
CREATE TABLE IF NOT EXISTS wf_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    asset_id UUID REFERENCES wf_assets(id),
    activity_type TEXT NOT NULL,
    activity_type_override TEXT,
    source_type TEXT,
    subtype TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING',
    activity_date DATE NOT NULL,
    settlement_date DATE,
    quantity DECIMAL(20, 10),
    unit_price DECIMAL(20, 10),
    amount DECIMAL(20, 10),
    fee DECIMAL(20, 10),
    currency TEXT NOT NULL DEFAULT 'EUR',
    fx_rate DECIMAL(20, 10),
    notes TEXT,
    metadata JSONB,
    source_system TEXT,
    source_record_id TEXT,
    source_group_id TEXT,
    idempotency_key TEXT,
    import_run_id UUID,
    is_user_modified BOOLEAN NOT NULL DEFAULT false,
    needs_review BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_activities_account ON wf_activities(account_id);
CREATE INDEX IF NOT EXISTS idx_wf_activities_asset ON wf_activities(asset_id);
CREATE INDEX IF NOT EXISTS idx_wf_activities_date ON wf_activities(activity_date);
CREATE INDEX IF NOT EXISTS idx_wf_activities_type ON wf_activities(activity_type);
CREATE INDEX IF NOT EXISTS idx_wf_activities_idempotency ON wf_activities(idempotency_key);
-- =====================================================
-- QUOTES & MARKET DATA
-- =====================================================

-- Quotes (price history)
CREATE TABLE IF NOT EXISTS wf_quotes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID NOT NULL REFERENCES wf_assets(id) ON DELETE CASCADE,
    day DATE NOT NULL,
    source TEXT NOT NULL DEFAULT 'MANUAL',
    open DECIMAL(20, 10),
    high DECIMAL(20, 10),
    low DECIMAL(20, 10),
    close DECIMAL(20, 10) NOT NULL,
    adjclose DECIMAL(20, 10),
    volume BIGINT,
    currency TEXT NOT NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    "timestamp" TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(asset_id, day, source)
);
CREATE INDEX IF NOT EXISTS idx_wf_quotes_asset_day ON wf_quotes(asset_id, day);
-- Quote Sync State
CREATE TABLE IF NOT EXISTS wf_quote_sync_state (
    asset_id UUID PRIMARY KEY REFERENCES wf_assets(id) ON DELETE CASCADE,
    position_closed_date DATE,
    last_synced_at TIMESTAMPTZ,
    data_source TEXT NOT NULL DEFAULT 'YAHOO',
    sync_priority INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    profile_enriched_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Market Data Providers
CREATE TABLE IF NOT EXISTS wf_market_data_providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    url TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    logo_filename TEXT,
    last_synced_at TIMESTAMPTZ,
    last_sync_status TEXT,
    last_sync_error TEXT
);
-- =====================================================
-- VALUATION & SNAPSHOTS
-- =====================================================

-- Daily Account Valuation
CREATE TABLE IF NOT EXISTS wf_daily_account_valuation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    valuation_date DATE NOT NULL,
    account_currency TEXT NOT NULL,
    base_currency TEXT NOT NULL,
    fx_rate_to_base DECIMAL(20, 10) NOT NULL,
    cash_balance DECIMAL(20, 10) NOT NULL,
    investment_market_value DECIMAL(20, 10) NOT NULL,
    total_value DECIMAL(20, 10) NOT NULL,
    cost_basis DECIMAL(20, 10) NOT NULL,
    net_contribution DECIMAL(20, 10) NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(account_id, valuation_date)
);
CREATE INDEX IF NOT EXISTS idx_wf_valuation_account_date ON wf_daily_account_valuation(account_id, valuation_date);
-- Holdings Snapshots
CREATE TABLE IF NOT EXISTS wf_holdings_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    snapshot_date DATE NOT NULL,
    currency TEXT NOT NULL,
    positions JSONB NOT NULL,
    cash_balances JSONB NOT NULL,
    cost_basis DECIMAL(20, 10) NOT NULL,
    net_contribution DECIMAL(20, 10) NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    net_contribution_base DECIMAL(20, 10) NOT NULL,
    cash_total_account_currency DECIMAL(20, 10) NOT NULL,
    cash_total_base_currency DECIMAL(20, 10) NOT NULL,
    source TEXT NOT NULL DEFAULT 'CALCULATED',
    UNIQUE(account_id, snapshot_date)
);
CREATE INDEX IF NOT EXISTS idx_wf_holdings_account_date ON wf_holdings_snapshots(account_id, snapshot_date);
-- =====================================================
-- IMPORT & SYNC
-- =====================================================

-- Import Runs
CREATE TABLE IF NOT EXISTS wf_import_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    source_system TEXT NOT NULL,
    run_type TEXT NOT NULL,
    mode TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'RUNNING',
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    review_mode TEXT NOT NULL DEFAULT 'AUTO',
    applied_at TIMESTAMPTZ,
    checkpoint_in JSONB,
    checkpoint_out JSONB,
    summary JSONB,
    warnings JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_import_runs_account ON wf_import_runs(account_id);
-- Activity Import Profiles
CREATE TABLE IF NOT EXISTS wf_activity_import_profiles (
    account_id UUID PRIMARY KEY REFERENCES wf_accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    config JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Brokers Sync State
CREATE TABLE IF NOT EXISTS wf_brokers_sync_state (
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    checkpoint_json JSONB,
    last_attempted_at TIMESTAMPTZ,
    last_successful_at TIMESTAMPTZ,
    last_error TEXT,
    last_run_id UUID REFERENCES wf_import_runs(id),
    sync_status TEXT NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, provider)
);
-- =====================================================
-- TAXONOMY & CLASSIFICATION
-- =====================================================

-- Taxonomies
CREATE TABLE IF NOT EXISTS wf_taxonomies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6366f1',
    description TEXT,
    is_system BOOLEAN NOT NULL DEFAULT false,
    is_single_select BOOLEAN NOT NULL DEFAULT true,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Taxonomy Categories
CREATE TABLE IF NOT EXISTS wf_taxonomy_categories (
    id UUID NOT NULL,
    taxonomy_id UUID NOT NULL REFERENCES wf_taxonomies(id) ON DELETE CASCADE,
    parent_id UUID,
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6366f1',
    description TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, taxonomy_id)
);
CREATE INDEX IF NOT EXISTS idx_wf_taxonomy_cat_parent ON wf_taxonomy_categories(parent_id);
-- Asset Taxonomy Assignments
CREATE TABLE IF NOT EXISTS wf_asset_taxonomy_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID NOT NULL REFERENCES wf_assets(id) ON DELETE CASCADE,
    taxonomy_id UUID NOT NULL REFERENCES wf_taxonomies(id) ON DELETE CASCADE,
    category_id UUID NOT NULL,
    weight INTEGER NOT NULL DEFAULT 100,
    source TEXT NOT NULL DEFAULT 'USER',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(asset_id, taxonomy_id, category_id)
);
CREATE INDEX IF NOT EXISTS idx_wf_asset_tax_asset ON wf_asset_taxonomy_assignments(asset_id);
-- =====================================================
-- GOALS
-- =====================================================

-- Goals
CREATE TABLE IF NOT EXISTS wf_goals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT,
    target_amount DECIMAL(20, 2) NOT NULL,
    is_achieved BOOLEAN NOT NULL DEFAULT false
);
-- Goals Allocation
CREATE TABLE IF NOT EXISTS wf_goals_allocation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    percent_allocation INTEGER NOT NULL DEFAULT 100,
    goal_id UUID NOT NULL REFERENCES wf_goals(id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    UNIQUE(goal_id, account_id)
);
-- Contribution Limits
CREATE TABLE IF NOT EXISTS wf_contribution_limits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_name TEXT NOT NULL,
    contribution_year INTEGER NOT NULL,
    limit_amount DECIMAL(20, 2) NOT NULL,
    account_ids JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    start_date DATE,
    end_date DATE
);
-- =====================================================
-- AI ASSISTANT (Wealthfolio format)
-- =====================================================

-- AI Threads
CREATE TABLE IF NOT EXISTS wf_ai_threads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    config_snapshot JSONB,
    is_pinned BOOLEAN NOT NULL DEFAULT false
);
-- AI Messages
CREATE TABLE IF NOT EXISTS wf_ai_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id UUID NOT NULL REFERENCES wf_ai_threads(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_ai_messages_thread ON wf_ai_messages(thread_id);
-- AI Thread Tags
CREATE TABLE IF NOT EXISTS wf_ai_thread_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id UUID NOT NULL REFERENCES wf_ai_threads(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_ai_thread_tags_thread ON wf_ai_thread_tags(thread_id);
-- =====================================================
-- SETTINGS & CONFIG
-- =====================================================

-- App Settings
CREATE TABLE IF NOT EXISTS wf_app_settings (
    setting_key TEXT PRIMARY KEY,
    setting_value TEXT NOT NULL
);
-- Health Issue Dismissals
CREATE TABLE IF NOT EXISTS wf_health_issue_dismissals (
    issue_id TEXT PRIMARY KEY,
    dismissed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    data_hash TEXT NOT NULL
);
-- =====================================================
-- USERS (Wealthfolio format - complement to profiles)
-- =====================================================

-- Users
CREATE TABLE IF NOT EXISTS wf_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    full_name TEXT,
    avatar_url TEXT,
    preferred_currency TEXT DEFAULT 'EUR',
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ
);
-- User Roles (Wealthfolio format)
CREATE TABLE IF NOT EXISTS wf_user_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES wf_users(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by UUID
);
CREATE INDEX IF NOT EXISTS idx_wf_user_roles_user ON wf_user_roles(user_id);
-- =====================================================
-- DEVICE SYNC (for multi-device support)
-- =====================================================

-- Sync Cursor
CREATE TABLE IF NOT EXISTS wf_sync_cursor (
    id INTEGER PRIMARY KEY DEFAULT 1,
    cursor BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Sync Device Config
CREATE TABLE IF NOT EXISTS wf_sync_device_config (
    device_id TEXT PRIMARY KEY,
    key_version INTEGER,
    trust_state TEXT NOT NULL DEFAULT 'PENDING',
    last_bootstrap_at TIMESTAMPTZ
);
-- Sync Engine State
CREATE TABLE IF NOT EXISTS wf_sync_engine_state (
    id INTEGER PRIMARY KEY DEFAULT 1,
    lock_version BIGINT NOT NULL DEFAULT 0,
    last_push_at TIMESTAMPTZ,
    last_pull_at TIMESTAMPTZ,
    last_error TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_cycle_status TEXT,
    last_cycle_duration_ms BIGINT
);
-- Sync Entity Metadata
CREATE TABLE IF NOT EXISTS wf_sync_entity_metadata (
    entity TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    last_event_id TEXT NOT NULL,
    last_client_timestamp TIMESTAMPTZ NOT NULL,
    last_seq BIGINT NOT NULL,
    PRIMARY KEY (entity, entity_id)
);
-- Sync Outbox
CREATE TABLE IF NOT EXISTS wf_sync_outbox (
    event_id TEXT PRIMARY KEY,
    entity TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    op TEXT NOT NULL,
    client_timestamp TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL,
    payload_key_version INTEGER NOT NULL DEFAULT 1,
    sent BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL DEFAULT 'PENDING',
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ,
    last_error TEXT,
    last_error_code TEXT,
    device_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wf_sync_outbox_sent ON wf_sync_outbox(sent, status);
-- Sync Applied Events
CREATE TABLE IF NOT EXISTS wf_sync_applied_events (
    event_id TEXT PRIMARY KEY,
    seq BIGINT NOT NULL,
    entity TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Sync Table State
CREATE TABLE IF NOT EXISTS wf_sync_table_state (
    table_name TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_snapshot_restore_at TIMESTAMPTZ,
    last_incremental_apply_at TIMESTAMPTZ
);
-- =====================================================
-- REVOLUT INTEGRATION (Wealthfolio format)
-- Keep separate from Orus revolut tables for now
-- =====================================================

-- Wealthfolio Revolut Accounts (linked to wf_accounts)
CREATE TABLE IF NOT EXISTS wf_revolut_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revolut_account_id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    currency TEXT NOT NULL,
    balance DECIMAL(20, 10) NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'ACTIVE',
    wf_account_id UUID REFERENCES wf_accounts(id),
    last_synced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Wealthfolio Revolut Auth Tokens
CREATE TABLE IF NOT EXISTS wf_revolut_auth_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    token_type TEXT NOT NULL DEFAULT 'Bearer',
    scope TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Wealthfolio Revolut Sync Log
CREATE TABLE IF NOT EXISTS wf_revolut_sync_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sync_type TEXT NOT NULL,
    status TEXT NOT NULL,
    accounts_synced INTEGER,
    transactions_synced INTEGER,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
-- Wealthfolio Revolut Transactions
CREATE TABLE IF NOT EXISTS wf_revolut_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revolut_transaction_id TEXT NOT NULL UNIQUE,
    revolut_account_id TEXT NOT NULL,
    tx_type TEXT NOT NULL,
    state TEXT NOT NULL,
    amount DECIMAL(20, 10) NOT NULL,
    currency TEXT NOT NULL,
    description TEXT,
    merchant_name TEXT,
    merchant_category TEXT,
    reference TEXT,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    balance_after DECIMAL(20, 10),
    wf_activity_id UUID REFERENCES wf_activities(id),
    synced_to_activities BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_wf_revolut_tx_account ON wf_revolut_transactions(revolut_account_id);
-- =====================================================
-- Add import_run_id foreign key to activities
-- =====================================================
ALTER TABLE wf_activities 
    ADD CONSTRAINT fk_activities_import_run 
    FOREIGN KEY (import_run_id) 
    REFERENCES wf_import_runs(id);

-- Enable RLS on all tables
ALTER TABLE wf_platforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_assets ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_activities ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_quotes ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_quote_sync_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_market_data_providers ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_daily_account_valuation ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_holdings_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_import_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_activity_import_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_brokers_sync_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_taxonomies ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_taxonomy_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_asset_taxonomy_assignments ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_goals ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_goals_allocation ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_contribution_limits ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_ai_threads ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_ai_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_ai_thread_tags ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_app_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_health_issue_dismissals ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_users ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_user_roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_cursor ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_device_config ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_engine_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_entity_metadata ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_outbox ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_applied_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_sync_table_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_revolut_accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_revolut_auth_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_revolut_sync_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_revolut_transactions ENABLE ROW LEVEL SECURITY;
