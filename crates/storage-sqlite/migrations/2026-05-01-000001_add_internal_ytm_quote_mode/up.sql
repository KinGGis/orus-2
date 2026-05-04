-- Add INTERNAL_YTM to the allowed quote_mode values for the assets table.
-- SQLite does not support ALTER CHECK, so we rebuild the table.

PRAGMA foreign_keys = OFF;

ALTER TABLE assets RENAME TO assets_old_pre_ytm;

CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL DEFAULT (lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' || '4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))),

    -- Core identity
    kind TEXT NOT NULL,
    name TEXT,
    display_code TEXT,
    notes TEXT,
    metadata TEXT,

    is_active INTEGER NOT NULL DEFAULT 1,

    -- Valuation
    quote_mode TEXT NOT NULL,             -- MARKET | MANUAL | INTERNAL_YTM
    quote_ccy TEXT NOT NULL,

    -- Instrument identity (NULL for non-market assets)
    instrument_type TEXT,
    instrument_symbol TEXT,
    instrument_exchange_mic TEXT,

    -- Computed canonical key (materialized on disk, never set directly)
    instrument_key TEXT GENERATED ALWAYS AS (
        CASE
            WHEN instrument_type IS NULL OR instrument_symbol IS NULL THEN NULL
            WHEN instrument_type IN ('FX', 'CRYPTO')
                THEN instrument_type || ':' || instrument_symbol || '/' || quote_ccy
            WHEN instrument_exchange_mic IS NOT NULL
                THEN instrument_type || ':' || instrument_symbol || '@' || instrument_exchange_mic
            ELSE instrument_type || ':' || instrument_symbol
        END
    ) STORED,

    -- Provider configuration (single JSON blob)
    provider_config TEXT,

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (kind IN (
        'INVESTMENT',
        'PROPERTY', 'VEHICLE', 'COLLECTIBLE', 'PRECIOUS_METAL',
        'PRIVATE_EQUITY', 'LIABILITY', 'OTHER',
        'FX'
    )),
    CHECK (quote_mode IN ('MARKET', 'MANUAL', 'INTERNAL_YTM')),
    CHECK (is_active IN (0, 1)),
    CHECK (metadata IS NULL OR json_valid(metadata)),
    CHECK (provider_config IS NULL OR json_valid(provider_config))
);

INSERT INTO assets (
    id, kind, name, display_code, notes, metadata,
    is_active, quote_mode, quote_ccy,
    instrument_type, instrument_symbol, instrument_exchange_mic,
    provider_config, created_at, updated_at
)
SELECT
    id, kind, name, display_code, notes, metadata,
    is_active, quote_mode, quote_ccy,
    instrument_type, instrument_symbol, instrument_exchange_mic,
    provider_config, created_at, updated_at
FROM assets_old_pre_ytm;

DROP TABLE assets_old_pre_ytm;

-- Recreate indexes
CREATE UNIQUE INDEX idx_assets_instrument_key
ON assets(instrument_key)
WHERE instrument_key IS NOT NULL;

CREATE INDEX idx_assets_kind ON assets(kind);
CREATE INDEX idx_assets_is_active ON assets(is_active);
CREATE INDEX idx_assets_display_code ON assets(display_code);

-- IMPORTANT: SQLite's `ALTER TABLE ... RENAME` does NOT update FOREIGN KEY
-- references stored in child tables' CREATE TABLE statements. After the
-- rename + recreate dance above, child tables (`activities`, `quotes`,
-- `asset_taxonomy_assignments`) still textually reference the old name
-- `"assets_old_pre_ytm"`, even though their actual rows still resolve
-- correctly because the new `assets` table preserves the same `id` values.
-- That mismatch breaks any DELETE / cascade evaluation with
-- `no such table: main.assets_old_pre_ytm`. Patch sqlite_master in place
-- to rewrite the FK clauses without touching row data.
PRAGMA writable_schema = ON;
UPDATE sqlite_master
SET sql = replace(sql, '"assets_old_pre_ytm"', 'assets')
WHERE type = 'table'
  AND name IN ('activities', 'quotes', 'asset_taxonomy_assignments')
  AND sql LIKE '%assets_old_pre_ytm%';
UPDATE sqlite_master
SET sql = replace(sql, 'assets_old_pre_ytm', 'assets')
WHERE type = 'table'
  AND name IN ('activities', 'quotes', 'asset_taxonomy_assignments')
  AND sql LIKE '%assets_old_pre_ytm%';
PRAGMA writable_schema = OFF;

PRAGMA foreign_keys = ON;
