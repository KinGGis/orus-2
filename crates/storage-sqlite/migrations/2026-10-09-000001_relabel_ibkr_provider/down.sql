-- Restore the previous, inaccurate label.

UPDATE accounts
SET provider = 'SNAPTRADE'
WHERE provider_account_id = 'IBKR-MCP-PRIMARY';
