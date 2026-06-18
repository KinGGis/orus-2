use std::{net::SocketAddr, time::Duration};

use crate::auth::{
    decode_secret_key, derive_keys, AuthConfig, CookieSecurePolicy, OrusAuthConfig,
};

const DEFAULT_ORUS_SUPABASE_URL: &str = "https://bujfwfqmfsgaibnmuvml.supabase.co";
const DEFAULT_ORUS_SUPABASE_ANON_KEY: &str =
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImJ1amZ3ZnFtZnNnYWlibm11dm1sIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NDUyNzUyMzYsImV4cCI6MjA2MDg1MTIzNn0.5dzs-01w-2tm21tfDmJIHzHFAnjBEKCapZfLSBLUiUo";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageBackend {
    Sqlite,
    Postgres,
}

impl StorageBackend {
    fn from_env() -> Self {
        let raw = std::env::var("WF_STORAGE_BACKEND").unwrap_or_else(|_| "sqlite".into());
        match raw.trim().to_ascii_lowercase().as_str() {
            "sqlite" => Self::Sqlite,
            "postgres" => Self::Postgres,
            other => panic!(
                "Invalid WF_STORAGE_BACKEND value: \"{other}\". Expected one of: sqlite, postgres"
            ),
        }
    }
}

pub struct Config {
    pub listen_addr: SocketAddr,
    pub storage_backend: StorageBackend,
    pub db_path: String,
    pub database_url: Option<String>,
    pub cors_allow: Vec<String>,
    pub request_timeout: Duration,
    pub static_dir: String,
    pub addons_root: String,
    /// Raw master key (used only for secret-store migration from old raw key)
    pub raw_secret_key: Vec<u8>,
    /// HKDF-derived key for secrets encryption
    pub secrets_encryption_key: [u8; 32],
    pub auth: Option<AuthConfig>,
    pub orus_auth: Option<OrusAuthConfig>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        let listen_addr: SocketAddr = std::env::var("WF_LISTEN_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8088".to_string())
            .parse()
            .expect("Invalid WF_LISTEN_ADDR");
        let storage_backend = StorageBackend::from_env();
        let db_path = std::env::var("WF_DB_PATH").unwrap_or_else(|_| "./db/app.db".into());
        let database_url = match storage_backend {
            StorageBackend::Sqlite => None,
            StorageBackend::Postgres => Some(
                std::env::var("DATABASE_URL")
                    .ok()
                    .or_else(|| std::env::var("SUPABASE_DB_URL").ok())
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| {
                        panic!(
                            "WF_STORAGE_BACKEND=postgres requires DATABASE_URL or SUPABASE_DB_URL"
                        )
                    }),
            ),
        };
        let cors_allow: Vec<String> = std::env::var("WF_CORS_ALLOW_ORIGINS")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "*".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let timeout_ms: u64 = std::env::var("WF_REQUEST_TIMEOUT_MS")
            .unwrap_or_else(|_| "30000".into())
            .parse()
            .unwrap_or(30000);
        let static_dir = std::env::var("WF_STATIC_DIR").unwrap_or_else(|_| "dist".into());
        let secret_key = std::env::var("WF_SECRET_KEY")
            .unwrap_or_else(|_| panic!("WF_SECRET_KEY must be set and contain a 32-byte key"))
            .trim()
            .to_string();
        if secret_key.is_empty() {
            panic!("WF_SECRET_KEY must not be empty");
        }
        let raw_secret_key = decode_secret_key(&secret_key)
            .unwrap_or_else(|e| panic!("Failed to decode WF_SECRET_KEY: {e}"));
        let (jwt_key, secrets_encryption_key) = derive_keys(&raw_secret_key);
        let addons_root = std::env::var("WF_ADDONS_DIR").unwrap_or_else(|_| {
            std::path::Path::new(&db_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_string_lossy()
                .into_owned()
        });
        let orus_auth_enabled = std::env::var("WF_ORUS_AUTH")
            .map(|value| !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let orus_auth = orus_auth_enabled.then(|| OrusAuthConfig {
            supabase_url: std::env::var("ORUS_SUPABASE_URL")
                .unwrap_or_else(|_| DEFAULT_ORUS_SUPABASE_URL.into())
                .trim()
                .to_string(),
            supabase_anon_key: std::env::var("ORUS_SUPABASE_ANON_KEY")
                .unwrap_or_else(|_| DEFAULT_ORUS_SUPABASE_ANON_KEY.into())
                .trim()
                .to_string(),
        });
        let password_hash = std::env::var("WF_AUTH_PASSWORD_HASH")
            .ok()
            .map(|hash| hash.trim().to_string())
            .filter(|hash| !hash.is_empty());
        let auth = if password_hash.is_some() || orus_auth.is_some() {
            Some({
                let ttl_minutes = std::env::var("WF_AUTH_TOKEN_TTL_MINUTES")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| *value > 0)
                    .unwrap_or(60);
                let cookie_secure_raw =
                    std::env::var("WF_COOKIE_SECURE").unwrap_or_else(|_| "auto".into());
                let cookie_secure = match cookie_secure_raw.trim().to_ascii_lowercase().as_str() {
                    "auto" => CookieSecurePolicy::Auto,
                    "true" | "1" | "yes" => CookieSecurePolicy::Always,
                    "false" | "0" | "no" => CookieSecurePolicy::Never,
                    other => panic!(
                        "Invalid WF_COOKIE_SECURE value: \"{other}\". \
                         Expected one of: auto, true, false"
                    ),
                };
                AuthConfig {
                    password_hash: password_hash.clone(),
                    jwt_secret: jwt_key.to_vec(),
                    access_token_ttl: Duration::from_secs(ttl_minutes.saturating_mul(60)),
                    cookie_secure,
                }
            })
        } else {
            None
        };
        // When auth is enabled, wildcard CORS is incompatible with credentials
        if auth.is_some() && cors_allow.iter().any(|o| o == "*") {
            panic!(
                "WF_CORS_ALLOW_ORIGINS cannot be \"*\" when authentication is enabled. \
                 Set explicit origins, e.g. WF_CORS_ALLOW_ORIGINS=https://my.domain.com"
            );
        }

        // Fail-closed: refuse to start on non-loopback without auth,
        // unless explicitly opted out via WF_AUTH_REQUIRED=false.
        if auth.is_none() && !listen_addr.ip().is_loopback() {
            let auth_required = std::env::var("WF_AUTH_REQUIRED")
                .map(|v| !v.eq_ignore_ascii_case("false"))
                .unwrap_or(true);
            if auth_required {
                panic!(
                    "Refusing to start: listening on non-loopback address {listen_addr} without \
                     authentication. Set WF_AUTH_PASSWORD_HASH to enable auth, or set \
                     WF_AUTH_REQUIRED=false if a reverse proxy handles authentication."
                );
            }
        }

        Self {
            listen_addr,
            storage_backend,
            db_path,
            database_url,
            cors_allow,
            request_timeout: Duration::from_millis(timeout_ms),
            static_dir,
            addons_root,
            raw_secret_key,
            secrets_encryption_key,
            auth,
            orus_auth,
        }
    }
}
