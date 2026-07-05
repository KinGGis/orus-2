-- Persistent secret storage for the Postgres backend.
-- On hosting without a persistent disk (e.g. Render free plan), the file-based
-- secret store at /data/secrets.json is wiped on every redeploy, so provider
-- API keys (Finnhub, Alpha Vantage, ...) and other secrets are lost. Storing
-- them in Postgres keeps them across deployments. Values are encrypted at rest
-- with WF_SECRET_KEY when it is configured.
CREATE TABLE IF NOT EXISTS wf_secrets (
    secret_key TEXT PRIMARY KEY,
    secret_value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
