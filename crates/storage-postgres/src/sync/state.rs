use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use serde_json::Value;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_brokers_sync_state::dsl as sync_state_dsl;
use wealthfolio_connect::broker_ingest::{
    BrokerSyncState, BrokerSyncStateRepositoryTrait as ConnectBrokerSyncStateRepositoryTrait,
    SyncStatus,
};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Debug,
    Clone,
)]
#[diesel(primary_key(account_id, provider))]
#[diesel(table_name = crate::schema::wf_brokers_sync_state)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct BrokerSyncStateDB {
    pub account_id: Uuid,
    pub provider: String,
    pub checkpoint_json: Option<Value>,
    pub last_attempted_at: Option<DateTime<Utc>>,
    pub last_successful_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_run_id: Option<Uuid>,
    pub sync_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn sync_status_to_db(value: SyncStatus) -> String {
    match value {
        SyncStatus::Idle => "IDLE",
        SyncStatus::Running => "RUNNING",
        SyncStatus::NeedsReview => "NEEDS_REVIEW",
        SyncStatus::Failed => "FAILED",
    }
    .to_string()
}

fn sync_status_from_db(value: &str) -> SyncStatus {
    match value {
        "RUNNING" | "SYNCING" => SyncStatus::Running,
        "NEEDS_REVIEW" => SyncStatus::NeedsReview,
        "FAILED" => SyncStatus::Failed,
        _ => SyncStatus::Idle,
    }
}

impl From<BrokerSyncStateDB> for BrokerSyncState {
    fn from(db: BrokerSyncStateDB) -> Self {
        Self {
            account_id: db.account_id.to_string(),
            provider: db.provider,
            checkpoint_json: db.checkpoint_json,
            last_attempted_at: db.last_attempted_at,
            last_successful_at: db.last_successful_at,
            last_error: db.last_error,
            last_run_id: db.last_run_id.map(|value| value.to_string()),
            sync_status: sync_status_from_db(&db.sync_status),
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl TryFrom<BrokerSyncState> for BrokerSyncStateDB {
    type Error = Error;

    fn try_from(domain: BrokerSyncState) -> Result<Self> {
        Ok(Self {
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            provider: domain.provider,
            checkpoint_json: domain.checkpoint_json,
            last_attempted_at: domain.last_attempted_at,
            last_successful_at: domain.last_successful_at,
            last_error: domain.last_error,
            last_run_id: domain
                .last_run_id
                .as_deref()
                .map(|value| parse_uuid(value, "import_run_id"))
                .transpose()?,
            sync_status: sync_status_to_db(domain.sync_status),
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        })
    }
}

pub struct BrokerSyncStateRepository {
    pool: Arc<DbPool>,
}

impl BrokerSyncStateRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConnectBrokerSyncStateRepositoryTrait for BrokerSyncStateRepository {
    fn get_by_account_id(&self, account_id: &str) -> Result<Option<BrokerSyncState>> {
        let account_uuid = parse_uuid(account_id, "account_id")?;
        let mut conn = get_connection(&self.pool)?;

        let result = sync_state_dsl::wf_brokers_sync_state
            .filter(sync_state_dsl::account_id.eq(account_uuid))
            .order(sync_state_dsl::updated_at.desc())
            .select(BrokerSyncStateDB::as_select())
            .first::<BrokerSyncStateDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    async fn upsert_attempt(&self, account_id: String, provider: String) -> Result<()> {
        let now = Utc::now();
        let account_uuid = parse_uuid(&account_id, "account_id")?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(sync_state_dsl::wf_brokers_sync_state)
            .values((
                sync_state_dsl::account_id.eq(account_uuid),
                sync_state_dsl::provider.eq(provider.clone()),
                sync_state_dsl::checkpoint_json.eq::<Option<Value>>(None),
                sync_state_dsl::last_attempted_at.eq(Some(now)),
                sync_state_dsl::last_successful_at.eq::<Option<DateTime<Utc>>>(None),
                sync_state_dsl::last_error.eq::<Option<String>>(None),
                sync_state_dsl::last_run_id.eq::<Option<Uuid>>(None),
                sync_state_dsl::sync_status.eq("RUNNING"),
                sync_state_dsl::created_at.eq(now),
                sync_state_dsl::updated_at.eq(now),
            ))
            .on_conflict((sync_state_dsl::account_id, sync_state_dsl::provider))
            .do_update()
            .set((
                sync_state_dsl::last_attempted_at.eq(excluded(sync_state_dsl::last_attempted_at)),
                sync_state_dsl::sync_status.eq("RUNNING"),
                sync_state_dsl::updated_at.eq(excluded(sync_state_dsl::updated_at)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn upsert_success(
        &self,
        account_id: String,
        provider: String,
        _last_synced_date: String,
        import_run_id: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();
        let account_uuid = parse_uuid(&account_id, "account_id")?;
        let run_uuid = import_run_id
            .as_deref()
            .map(|value| parse_uuid(value, "import_run_id"))
            .transpose()?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(sync_state_dsl::wf_brokers_sync_state)
            .values((
                sync_state_dsl::account_id.eq(account_uuid),
                sync_state_dsl::provider.eq(provider.clone()),
                sync_state_dsl::checkpoint_json.eq::<Option<Value>>(None),
                sync_state_dsl::last_attempted_at.eq(Some(now)),
                sync_state_dsl::last_successful_at.eq(Some(now)),
                sync_state_dsl::last_error.eq::<Option<String>>(None),
                sync_state_dsl::last_run_id.eq(run_uuid),
                sync_state_dsl::sync_status.eq("IDLE"),
                sync_state_dsl::created_at.eq(now),
                sync_state_dsl::updated_at.eq(now),
            ))
            .on_conflict((sync_state_dsl::account_id, sync_state_dsl::provider))
            .do_update()
            .set((
                sync_state_dsl::last_successful_at.eq(excluded(sync_state_dsl::last_successful_at)),
                sync_state_dsl::last_error.eq::<Option<String>>(None),
                sync_state_dsl::last_run_id.eq(excluded(sync_state_dsl::last_run_id)),
                sync_state_dsl::sync_status.eq("IDLE"),
                sync_state_dsl::updated_at.eq(excluded(sync_state_dsl::updated_at)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn upsert_failure(
        &self,
        account_id: String,
        provider: String,
        error: String,
        import_run_id: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();
        let account_uuid = parse_uuid(&account_id, "account_id")?;
        let run_uuid = import_run_id
            .as_deref()
            .map(|value| parse_uuid(value, "import_run_id"))
            .transpose()?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(sync_state_dsl::wf_brokers_sync_state)
            .values((
                sync_state_dsl::account_id.eq(account_uuid),
                sync_state_dsl::provider.eq(provider.clone()),
                sync_state_dsl::checkpoint_json.eq::<Option<Value>>(None),
                sync_state_dsl::last_attempted_at.eq(Some(now)),
                sync_state_dsl::last_successful_at.eq::<Option<DateTime<Utc>>>(None),
                sync_state_dsl::last_error.eq(Some(error.clone())),
                sync_state_dsl::last_run_id.eq(run_uuid),
                sync_state_dsl::sync_status.eq("FAILED"),
                sync_state_dsl::created_at.eq(now),
                sync_state_dsl::updated_at.eq(now),
            ))
            .on_conflict((sync_state_dsl::account_id, sync_state_dsl::provider))
            .do_update()
            .set((
                sync_state_dsl::last_error.eq(excluded(sync_state_dsl::last_error)),
                sync_state_dsl::last_run_id.eq(excluded(sync_state_dsl::last_run_id)),
                sync_state_dsl::sync_status.eq("FAILED"),
                sync_state_dsl::updated_at.eq(excluded(sync_state_dsl::updated_at)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn upsert_needs_review(
        &self,
        account_id: String,
        provider: String,
        warning: String,
        import_run_id: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();
        let account_uuid = parse_uuid(&account_id, "account_id")?;
        let run_uuid = import_run_id
            .as_deref()
            .map(|value| parse_uuid(value, "import_run_id"))
            .transpose()?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(sync_state_dsl::wf_brokers_sync_state)
            .values((
                sync_state_dsl::account_id.eq(account_uuid),
                sync_state_dsl::provider.eq(provider.clone()),
                sync_state_dsl::checkpoint_json.eq::<Option<Value>>(None),
                sync_state_dsl::last_attempted_at.eq(Some(now)),
                sync_state_dsl::last_successful_at.eq::<Option<DateTime<Utc>>>(None),
                sync_state_dsl::last_error.eq(Some(warning.clone())),
                sync_state_dsl::last_run_id.eq(run_uuid),
                sync_state_dsl::sync_status.eq("NEEDS_REVIEW"),
                sync_state_dsl::created_at.eq(now),
                sync_state_dsl::updated_at.eq(now),
            ))
            .on_conflict((sync_state_dsl::account_id, sync_state_dsl::provider))
            .do_update()
            .set((
                sync_state_dsl::last_error.eq(excluded(sync_state_dsl::last_error)),
                sync_state_dsl::last_run_id.eq(excluded(sync_state_dsl::last_run_id)),
                sync_state_dsl::sync_status.eq("NEEDS_REVIEW"),
                sync_state_dsl::updated_at.eq(excluded(sync_state_dsl::updated_at)),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn get_all(&self) -> Result<Vec<BrokerSyncState>> {
        let mut conn = get_connection(&self.pool)?;

        let results = sync_state_dsl::wf_brokers_sync_state
            .order(sync_state_dsl::updated_at.desc())
            .select(BrokerSyncStateDB::as_select())
            .load::<BrokerSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}