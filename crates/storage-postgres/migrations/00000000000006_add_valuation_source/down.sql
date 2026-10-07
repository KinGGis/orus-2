DROP INDEX IF EXISTS idx_wf_daily_account_valuation_source;

ALTER TABLE wf_daily_account_valuation
DROP COLUMN source;
