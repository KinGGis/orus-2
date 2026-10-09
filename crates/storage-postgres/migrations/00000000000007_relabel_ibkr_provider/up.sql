-- Name the integration that actually feeds the IBKR account.
--
-- The broker sync pipeline was written when the aggregator was the only
-- integration, so it stamped every account it created with 'SNAPTRADE'. The
-- direct IBKR connection reuses that pipeline, which left its account claiming
-- to be a SnapTrade one. The UI keys its refresh action off this column, so the
-- account offered a SnapTrade sync that does nothing for IBKR data.
--
-- Accounts are only ever created by the sync, never updated, so the label
-- cannot be repaired by syncing again.

UPDATE wf_accounts
SET provider = 'IBKR_MCP',
    updated_at = NOW()
WHERE provider_account_id = 'IBKR-MCP-PRIMARY'
  AND (provider IS NULL OR provider <> 'IBKR_MCP');
