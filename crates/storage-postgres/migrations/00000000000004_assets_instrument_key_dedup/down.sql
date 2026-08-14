-- Revert of 00000000000004_assets_instrument_key_dedup.
--
-- READ THIS BEFORE RELYING ON IT.
--
-- This is a PARTIAL rollback. It restores the schema and the deleted asset
-- rows, and it puts every activity back on the asset it originally pointed to
-- (recoverable only because up.sql recorded the original assignment in
-- wf_activities_dedup_map).
--
-- What it CANNOT restore:
--   * wf_quotes and wf_quote_sync_state rows destroyed by the ON DELETE CASCADE
--     of the duplicate purge. These are re-derivable: run a market data sync.
--   * wf_holdings_snapshots and wf_daily_account_valuation. Re-derivable: run
--     POST /api/v1/portfolio/recalculate.
--
-- So this script recovers the authoritative data (assets, activities) and
-- leaves the derived data to be recomputed. It is NOT a substitute for a
-- database backup. Take a Supabase snapshot before deploying the migration:
-- that snapshot, not this file, is the real safety net.

SET statement_timeout = 0;

-- ---------------------------------------------------------------------------
-- 1. Back to a plain, writable TEXT column.
-- ---------------------------------------------------------------------------

DROP INDEX IF EXISTS uq_wf_assets_instrument_key;

ALTER TABLE wf_assets DROP COLUMN IF EXISTS instrument_key;

ALTER TABLE wf_assets ADD COLUMN instrument_key TEXT;

CREATE INDEX IF NOT EXISTS idx_wf_assets_key ON wf_assets(instrument_key);

-- ---------------------------------------------------------------------------
-- 2. Restore the deleted duplicate rows from the snapshot taken by up.sql.
-- ---------------------------------------------------------------------------

INSERT INTO wf_assets (
    id, kind, name, display_code, notes, metadata, is_active,
    quote_mode, quote_ccy, instrument_type, instrument_symbol,
    instrument_exchange_mic, instrument_key, provider_config,
    created_at, updated_at
)
SELECT
    b.id, b.kind, b.name, b.display_code, b.notes, b.metadata, b.is_active,
    b.quote_mode, b.quote_ccy, b.instrument_type, b.instrument_symbol,
    b.instrument_exchange_mic, b.instrument_key, b.provider_config,
    b.created_at, b.updated_at
FROM wf_assets_dedup_backup b
WHERE NOT EXISTS (SELECT 1 FROM wf_assets a WHERE a.id = b.id);

-- ---------------------------------------------------------------------------
-- 3. Put every activity back on its original asset.
-- ---------------------------------------------------------------------------

UPDATE wf_activities a
SET asset_id = m.old_asset_id
FROM wf_activities_dedup_map m
WHERE a.id = m.activity_id;

-- ---------------------------------------------------------------------------
-- 4. Drop the helper tables so a re-run of up.sql starts from a clean slate.
-- ---------------------------------------------------------------------------

DROP TABLE IF EXISTS wf_activities_dedup_map;
DROP TABLE IF EXISTS wf_assets_dedup_map;
DROP TABLE IF EXISTS wf_assets_dedup_backup;
