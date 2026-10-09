-- Restore the previous, inaccurate label.

UPDATE wf_accounts
SET provider = 'SNAPTRADE'
WHERE provider_account_id = 'IBKR-MCP-PRIMARY';
