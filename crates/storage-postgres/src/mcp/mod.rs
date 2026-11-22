//! PostgreSQL persistence for the API keys of the MCP endpoint and the journal
//! of the tools AI agents call through it.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::schema::{wf_mcp_api_keys, wf_mcp_audit_log};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

/// An API key, without its secret: only its hash is stored.
#[derive(Queryable, Selectable, Serialize, Debug, Clone)]
#[diesel(table_name = wf_mcp_api_keys)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[serde(rename_all = "camelCase")]
pub struct McpApiKey {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    #[serde(skip)]
    pub key_hash: String,
    /// `READ` or `WRITE`.
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = wf_mcp_api_keys)]
pub struct NewMcpApiKey {
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub scope: String,
}

#[derive(Queryable, Selectable, Serialize, Debug, Clone)]
#[diesel(table_name = wf_mcp_audit_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[serde(rename_all = "camelCase")]
pub struct McpAuditEntry {
    pub id: Uuid,
    pub key_id: Option<Uuid>,
    pub tool_name: String,
    pub arguments: Option<serde_json::Value>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = wf_mcp_audit_log)]
pub struct NewMcpAuditEntry {
    pub key_id: Option<Uuid>,
    pub tool_name: String,
    pub arguments: Option<serde_json::Value>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: i32,
}

fn repo_err(err: diesel::result::Error) -> Error {
    Error::Repository(err.to_string())
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid key id: {err}"
        )))
    })
}

pub struct McpKeyRepository {
    pool: Arc<DbPool>,
}

impl McpKeyRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    pub fn create_key(&self, key: NewMcpApiKey) -> Result<McpApiKey> {
        let mut conn = get_connection(&self.pool)?;
        diesel::insert_into(wf_mcp_api_keys::table)
            .values(&key)
            .returning(McpApiKey::as_returning())
            .get_result(&mut conn)
            .map_err(repo_err)
    }

    /// Every key, revoked ones included, newest first.
    pub fn list_keys(&self) -> Result<Vec<McpApiKey>> {
        let mut conn = get_connection(&self.pool)?;
        wf_mcp_api_keys::table
            .order(wf_mcp_api_keys::created_at.desc())
            .select(McpApiKey::as_select())
            .load(&mut conn)
            .map_err(repo_err)
    }

    /// The non-revoked key with this hash, recording that it was just used.
    pub fn authenticate(&self, key_hash: &str) -> Result<Option<McpApiKey>> {
        let mut conn = get_connection(&self.pool)?;
        diesel::update(
            wf_mcp_api_keys::table
                .filter(wf_mcp_api_keys::key_hash.eq(key_hash))
                .filter(wf_mcp_api_keys::revoked_at.is_null()),
        )
        .set(wf_mcp_api_keys::last_used_at.eq(Utc::now()))
        .returning(McpApiKey::as_returning())
        .get_result(&mut conn)
        .optional()
        .map_err(repo_err)
    }

    /// Returns whether a still-active key was revoked.
    pub fn revoke_key(&self, id: &str) -> Result<bool> {
        let id = parse_uuid(id)?;
        let mut conn = get_connection(&self.pool)?;
        let updated = diesel::update(
            wf_mcp_api_keys::table
                .filter(wf_mcp_api_keys::id.eq(id))
                .filter(wf_mcp_api_keys::revoked_at.is_null()),
        )
        .set(wf_mcp_api_keys::revoked_at.eq(Utc::now()))
        .execute(&mut conn)
        .map_err(repo_err)?;
        Ok(updated > 0)
    }

    pub fn record_call(&self, entry: NewMcpAuditEntry) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;
        diesel::insert_into(wf_mcp_audit_log::table)
            .values(&entry)
            .execute(&mut conn)
            .map(|_| ())
            .map_err(repo_err)
    }

    /// The most recent tool calls, newest first.
    pub fn list_calls(&self, limit: i64) -> Result<Vec<McpAuditEntry>> {
        let mut conn = get_connection(&self.pool)?;
        wf_mcp_audit_log::table
            .order(wf_mcp_audit_log::created_at.desc())
            .limit(limit)
            .select(McpAuditEntry::as_select())
            .load(&mut conn)
            .map_err(repo_err)
    }
}
