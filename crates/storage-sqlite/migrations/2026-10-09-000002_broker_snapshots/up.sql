-- Historise what the broker reported, rather than reading it live.
--
-- Positions, trades, quotes and valuations are already persisted. The figures a
-- broker *derives* — its allocation breakdown, its account metrics, its period
-- returns — were fetched on every page render and never stored, so a page could
-- not be drawn while the broker was unreachable and nothing recorded what the
-- custodian actually reported on a given day. Track-record audits need that
-- record.
--
-- Each capture is stored twice on purpose: the payload verbatim, so an audit can
-- prove what the broker said and figures can be re-derived if our reading of
-- them changes, and one flattened row per category so the same history is
-- queryable in SQL.

CREATE TABLE broker_snapshots (
    id TEXT NOT NULL PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    kind TEXT NOT NULL,
    as_of_date TEXT NOT NULL,
    captured_at TEXT NOT NULL,
    payload TEXT NOT NULL,
    -- One capture per day per kind: syncing twice in a day refreshes that day
    -- instead of leaving two conflicting versions of it behind.
    UNIQUE (account_id, provider, kind, as_of_date)
);

CREATE INDEX idx_broker_snapshots_latest
ON broker_snapshots(account_id, kind, as_of_date DESC);

CREATE TABLE broker_snapshot_metrics (
    id TEXT NOT NULL PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES broker_snapshots(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    as_of_date TEXT NOT NULL,
    -- Names the breakdown: ASSET_CLASS, REGION, COUNTRY, SECTOR,
    -- ACCOUNT_SUMMARY or PERFORMANCE.
    dimension TEXT NOT NULL,
    -- Names the line within it: EQ, EUROPE, BUYING_POWER, YTD…
    category_id TEXT NOT NULL,
    category_name TEXT,
    -- Long and short legs are kept apart: netting them is lossy once leverage
    -- is involved.
    side TEXT NOT NULL,
    value TEXT,
    weight TEXT
);

CREATE INDEX idx_broker_snapshot_metrics_series
ON broker_snapshot_metrics(account_id, dimension, category_id, as_of_date);
