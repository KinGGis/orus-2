use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_import_runs::dsl as import_runs_dsl;
use wealthfolio_connect::broker_ingest::{
    ImportRun, ImportRunMode, ImportRunRepositoryTrait as ConnectImportRunRepositoryTrait,
    ImportRunStatus, ImportRunSummary, ImportRunType, ReviewMode,
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
#[diesel(table_name = crate::schema::wf_import_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct ImportRunDB {
    pub id: Uuid,
    pub account_id: Uuid,
    pub source_system: String,
    pub run_type: String,
    pub mode: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub review_mode: String,
    pub applied_at: Option<DateTime<Utc>>,
    pub checkpoint_in: Option<Value>,
    pub checkpoint_out: Option<Value>,
    pub summary: Option<Value>,
    pub warnings: Option<Value>,
    pub error: Option<String>,
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

fn enum_to_db<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn enum_from_db<T>(value: &str, fallback: T) -> T
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(&format!("\"{}\"", value)).unwrap_or(fallback)
}

fn parse_summary(value: Option<Value>) -> Option<ImportRunSummary> {
    value.and_then(|raw| serde_json::from_value(raw).ok())
}

fn parse_warnings(value: Option<Value>) -> Option<Vec<String>> {
    value.and_then(|raw| serde_json::from_value(raw).ok())
}

impl From<ImportRunDB> for ImportRun {
    fn from(db: ImportRunDB) -> Self {
        Self {
            id: db.id.to_string(),
            account_id: db.account_id.to_string(),
            source_system: db.source_system,
            run_type: enum_from_db(&db.run_type, ImportRunType::Sync),
            mode: enum_from_db(&db.mode, ImportRunMode::Incremental),
            status: enum_from_db(&db.status, ImportRunStatus::Running),
            started_at: db.started_at,
            finished_at: db.finished_at,
            review_mode: enum_from_db(&db.review_mode, ReviewMode::Never),
            applied_at: db.applied_at,
            checkpoint_in: db.checkpoint_in,
            checkpoint_out: db.checkpoint_out,
            summary: parse_summary(db.summary),
            warnings: parse_warnings(db.warnings),
            error: db.error,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl TryFrom<ImportRun> for ImportRunDB {
    type Error = Error;

    fn try_from(domain: ImportRun) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&domain.id, "import_run_id")?,
            account_id: parse_uuid(&domain.account_id, "account_id")?,
            source_system: domain.source_system,
            run_type: enum_to_db(&domain.run_type),
            mode: enum_to_db(&domain.mode),
            status: enum_to_db(&domain.status),
            started_at: domain.started_at,
            finished_at: domain.finished_at,
            review_mode: enum_to_db(&domain.review_mode),
            applied_at: domain.applied_at,
            checkpoint_in: domain.checkpoint_in,
            checkpoint_out: domain.checkpoint_out,
            summary: domain.summary.map(|value| serde_json::to_value(value).unwrap_or(Value::Null)),
            warnings: domain.warnings.map(|value| serde_json::to_value(value).unwrap_or(Value::Null)),
            error: domain.error,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        })
    }
}

pub struct ImportRunRepository {
    pool: Arc<DbPool>,
}

impl ImportRunRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn parse_account_id(account_id: &str) -> Result<Uuid> {
        parse_uuid(account_id, "account_id")
    }
}

#[async_trait]
impl ConnectImportRunRepositoryTrait for ImportRunRepository {
    async fn create(&self, import_run: ImportRun) -> Result<ImportRun> {
        let db_model = ImportRunDB::try_from(import_run)?;
        let mut conn = get_connection(&self.pool)?;

        let result = diesel::insert_into(import_runs_dsl::wf_import_runs)
            .values(&db_model)
            .returning(ImportRunDB::as_returning())
            .get_result::<ImportRunDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    async fn update(&self, import_run: ImportRun) -> Result<ImportRun> {
        let db_model = ImportRunDB::try_from(import_run)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::update(import_runs_dsl::wf_import_runs.find(db_model.id))
            .set(&db_model)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(db_model.into())
    }

    fn get_by_id(&self, id: &str) -> Result<Option<ImportRun>> {
        let import_run_id = parse_uuid(id, "import_run_id")?;
        let mut conn = get_connection(&self.pool)?;

        let result = import_runs_dsl::wf_import_runs
            .find(import_run_id)
            .select(ImportRunDB::as_select())
            .first::<ImportRunDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    fn get_recent_for_account(&self, account_id: &str, limit: i64) -> Result<Vec<ImportRun>> {
        let account_uuid = Self::parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;

        let results = import_runs_dsl::wf_import_runs
            .filter(import_runs_dsl::account_id.eq(account_uuid))
            .order(import_runs_dsl::started_at.desc())
            .limit(limit)
            .select(ImportRunDB::as_select())
            .load::<ImportRunDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    fn get_all(&self, limit: i64, offset: i64) -> Result<Vec<ImportRun>> {
        let mut conn = get_connection(&self.pool)?;

        let results = import_runs_dsl::wf_import_runs
            .order(import_runs_dsl::started_at.desc())
            .limit(limit)
            .offset(offset)
            .select(ImportRunDB::as_select())
            .load::<ImportRunDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    fn get_by_run_type(&self, run_type: &str, limit: i64, offset: i64) -> Result<Vec<ImportRun>> {
        let mut conn = get_connection(&self.pool)?;

        let results = import_runs_dsl::wf_import_runs
            .filter(import_runs_dsl::run_type.eq(run_type))
            .order(import_runs_dsl::started_at.desc())
            .limit(limit)
            .offset(offset)
            .select(ImportRunDB::as_select())
            .load::<ImportRunDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}