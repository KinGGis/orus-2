DROP INDEX IF EXISTS idx_daily_account_valuation_source;

ALTER TABLE daily_account_valuation
DROP COLUMN source;
