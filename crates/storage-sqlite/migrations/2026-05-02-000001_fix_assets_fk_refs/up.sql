-- Fix dangling foreign-key references created by the prior
-- 2026-05-01-000001_add_internal_ytm_quote_mode migration.
--
-- That migration renamed `assets` -> `assets_old_pre_ytm`, recreated
-- `assets`, then dropped the old table. SQLite does NOT rewrite child
-- tables' FOREIGN KEY clauses on RENAME, so `activities`, `quotes` and
-- `asset_taxonomy_assignments` still reference the (now non-existent)
-- table `"assets_old_pre_ytm"`. Any DELETE / cascade evaluation on those
-- tables fails with `no such table: main.assets_old_pre_ytm`.
--
-- We use `PRAGMA writable_schema` to patch sqlite_master in place. This
-- is the SQLite-recommended way to repair FK clauses without rebuilding
-- large child tables (especially `quotes`, which can have millions of
-- rows). The replacement is purely textual on the stored CREATE TABLE
-- statement; no row data is touched. Foreign-key enforcement is then
-- re-checked via PRAGMA foreign_key_check.

PRAGMA foreign_keys = OFF;

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
