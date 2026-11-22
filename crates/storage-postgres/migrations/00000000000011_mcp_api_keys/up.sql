-- API keys letting AI agents reach Orus through its MCP endpoint, and the
-- journal of every tool they call. Only a SHA-256 hash of each key is kept:
-- the key itself is shown once, when it is created.

CREATE TABLE wf_mcp_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    -- First characters of the key, so the owner can recognise it.
    key_prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    -- READ exposes data only; WRITE also allows operations.
    scope TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT wf_mcp_api_keys_scope_check CHECK (scope IN ('READ', 'WRITE'))
);

CREATE TABLE wf_mcp_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id UUID REFERENCES wf_mcp_api_keys(id) ON DELETE SET NULL,
    tool_name TEXT NOT NULL,
    arguments JSONB,
    success BOOLEAN NOT NULL,
    error TEXT,
    duration_ms INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX wf_mcp_audit_log_created_at_idx ON wf_mcp_audit_log (created_at DESC);