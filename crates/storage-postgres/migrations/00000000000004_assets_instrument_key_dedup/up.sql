-- Restore asset deduplication on the Postgres backend.
--
-- `instrument_key` is the sole deduplication key used by the application
-- (`AssetRepositoryTrait::find_by_instrument_key`). The Rust code never writes
-- it: on SQLite the column is `GENERATED ALWAYS AS (...) STORED` and the engine
-- fills it in. The Postgres DDL declared it as a plain `TEXT` column, so nothing
-- ever populated it, `find_by_instrument_key` never matched, and every broker
-- sync re-created the whole asset universe.
--
-- This migration merges the accumulated duplicates and then restores the
-- generated column plus the unique index that makes the drift impossible to
-- reproduce.
--
-- Diesel wraps each migration in a transaction and Postgres supports
-- transactional DDL: everything below either commits as a whole or is rolled
-- back entirely.

-- The duplicate purge cascades into wf_quotes (~100k rows). Render's default
-- statement timeout would abort it mid-way.
SET statement_timeout = 0;

-- ---------------------------------------------------------------------------
-- 1. Safety net: full snapshot + audit trail of every merge decision.
--    Both tables are kept after the migration; they are what makes the merge
--    auditable and (partially) reversible.
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS wf_assets_dedup_backup AS
SELECT * FROM wf_assets;

CREATE TABLE IF NOT EXISTS wf_assets_dedup_map (
    duplicate_id   UUID PRIMARY KEY,
    canonical_id   UUID NOT NULL,
    instrument_key TEXT NOT NULL,
    merged_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- 2. Resolve each duplicate group.
--
--    The key is computed inline with the exact expression used by SQLite
--    (crates/storage-sqlite/migrations/2026-01-01-000000_refactor_asset_model/up.sql:50),
--    since the column is not generated yet.
--
--    Canonical row = the MOST RECENT one. This is deliberate and differs from
--    the SQLite dedup migration, which keeps the oldest. On this database the
--    older rows carry the wrong instrument entirely (12 rows named "Big Tree
--    Cloud Holdings Ltd" under EQUITY:DSY@XPAR, against a single correct
--    "Dassault Systemes SE"). Keeping the oldest would enshrine the wrong name
--    and the wrong sector metadata on every merged asset.
--
--    Note on dual listings: rows are partitioned by the full key, so
--    EQUITY:STLA (NYSE, USD) and EQUITY:STLA@XPAR (Paris, EUR) stay separate.
--    They are genuinely distinct instruments and must never be merged.
-- ---------------------------------------------------------------------------

INSERT INTO wf_assets_dedup_map (duplicate_id, canonical_id, instrument_key)
WITH keyed AS (
    SELECT
        id,
        created_at,
        CASE
            WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
            WHEN instrument_type IN ('FX', 'CRYPTO')
                THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
            WHEN instrument_exchange_mic IS NOT NULL
                THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
            ELSE instrument_type || ':' || instrument_symbol
        END AS computed_key
    FROM wf_assets
),
ranked AS (
    SELECT
        id AS duplicate_id,
        computed_key,
        first_value(id) OVER (
            PARTITION BY computed_key
            ORDER BY created_at DESC, id DESC
        ) AS canonical_id,
        row_number() OVER (
            PARTITION BY computed_key
            ORDER BY created_at DESC, id DESC
        ) AS row_num
    FROM keyed
    WHERE computed_key IS NOT NULL
)
SELECT duplicate_id, canonical_id, computed_key
FROM ranked
WHERE row_num > 1
ON CONFLICT (duplicate_id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- 3. Re-point activities onto the canonical asset.
--
--    wf_activities.asset_id has NO ON DELETE CASCADE (see the reference DDL),
--    which is the safety property we rely on: if this step were to miss a row,
--    the DELETE in step 6 would raise a foreign key violation and abort the
--    migration rather than silently destroy transaction history.
-- ---------------------------------------------------------------------------

--    The duplicate -> canonical map is many-to-one, so it cannot tell which
--    duplicate a given activity came from. Record the original assignment
--    first, otherwise this step is not reversible.

CREATE TABLE IF NOT EXISTS wf_activities_dedup_map (
    activity_id  UUID PRIMARY KEY,
    old_asset_id UUID NOT NULL
);

INSERT INTO wf_activities_dedup_map (activity_id, old_asset_id)
SELECT a.id, a.asset_id
FROM wf_activities a
JOIN wf_assets_dedup_map m ON m.duplicate_id = a.asset_id
ON CONFLICT (activity_id) DO NOTHING;

UPDATE wf_activities a
SET asset_id = m.canonical_id
FROM wf_assets_dedup_map m
WHERE a.asset_id = m.duplicate_id;

-- ---------------------------------------------------------------------------
-- 4. Re-point taxonomy assignments.
--
--    UNIQUE(asset_id, taxonomy_id, category_id) means a naive UPDATE can
--    collide, either against a row already held by the canonical asset or
--    between two duplicates carrying the same category. Drop the losers first,
--    then move the survivors. Rows already attached to the canonical asset win
--    the tie-break.
--
--    This table is empty on the current database; the code is written for
--    correctness, not because rows are expected.
-- ---------------------------------------------------------------------------

DELETE FROM wf_asset_taxonomy_assignments t
USING (
    SELECT
        t2.id,
        row_number() OVER (
            PARTITION BY COALESCE(m.canonical_id, t2.asset_id), t2.taxonomy_id, t2.category_id
            ORDER BY (m.canonical_id IS NULL) DESC, t2.id
        ) AS row_num
    FROM wf_asset_taxonomy_assignments t2
    LEFT JOIN wf_assets_dedup_map m ON m.duplicate_id = t2.asset_id
) losers
WHERE t.id = losers.id
  AND losers.row_num > 1;

UPDATE wf_asset_taxonomy_assignments t
SET asset_id = m.canonical_id
FROM wf_assets_dedup_map m
WHERE t.asset_id = m.duplicate_id;

-- ---------------------------------------------------------------------------
-- 5. Quotes are deliberately NOT re-pointed.
--
--    Two reasons. First, wf_quotes carries UNIQUE(asset_id, day, source): a
--    bulk UPDATE would collide on every overlapping day. Second, and decisive:
--    the duplicates hold the price history of the WRONG company (Big Tree Cloud
--    quotes filed under DSY). Re-pointing would poison the canonical asset with
--    someone else's prices.
--
--    wf_quotes.asset_id and wf_quote_sync_state.asset_id are both
--    ON DELETE CASCADE, so step 6 discards them. A market data re-sync is
--    REQUIRED after this migration to rebuild the price history.
-- ---------------------------------------------------------------------------

-- ---------------------------------------------------------------------------
-- 6. Drop the duplicates.
-- ---------------------------------------------------------------------------

DELETE FROM wf_assets a
USING wf_assets_dedup_map m
WHERE a.id = m.duplicate_id;

-- ---------------------------------------------------------------------------
-- 7. Restore the generated column and the uniqueness guarantee.
--
--    The expression is character-for-character the SQLite one, so both backends
--    compute identical keys.
-- ---------------------------------------------------------------------------

DROP INDEX IF EXISTS idx_wf_assets_key;

ALTER TABLE wf_assets DROP COLUMN IF EXISTS instrument_key;

ALTER TABLE wf_assets
ADD COLUMN instrument_key TEXT GENERATED ALWAYS AS (
    CASE
        WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
        WHEN instrument_type IN ('FX', 'CRYPTO')
            THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
        WHEN instrument_exchange_mic IS NOT NULL
            THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
        ELSE instrument_type || ':' || instrument_symbol
    END
) STORED;

-- Mirrors idx_assets_instrument_key on SQLite. Created after the merge on
-- purpose: if any duplicate survived step 6, this fails and rolls the whole
-- migration back. That is the intended guard, not an accident.
CREATE UNIQUE INDEX uq_wf_assets_instrument_key
ON wf_assets(instrument_key)
WHERE instrument_key IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 8. Discard derived data computed against the now-deleted duplicates.
--    Holdings snapshots referenced duplicate asset ids exclusively, which is
--    what the UI was rendering. They must be recomputed
--    (POST /api/v1/portfolio/recalculate) after deployment.
-- ---------------------------------------------------------------------------

DELETE FROM wf_holdings_snapshots;
DELETE FROM wf_daily_account_valuation;
