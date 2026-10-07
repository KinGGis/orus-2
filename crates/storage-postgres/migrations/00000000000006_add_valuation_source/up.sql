-- Track where a daily valuation came from.
--
-- Broker-reported valuations are authoritative: unlike computed rows they are
-- not derived from local quotes, so a full recalculation must leave them in
-- place instead of replacing them with figures built from incomplete market
-- data.

ALTER TABLE wf_daily_account_valuation
ADD COLUMN source TEXT NOT NULL DEFAULT 'CALCULATED';

CREATE INDEX idx_wf_daily_account_valuation_source
ON wf_daily_account_valuation(account_id, source);
