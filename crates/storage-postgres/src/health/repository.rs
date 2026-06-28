use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::health::model::HealthIssueDismissalDB;
use crate::schema::wf_health_issue_dismissals;
use crate::schema::wf_health_issue_dismissals::dsl::*;
use wealthfolio_core::health::{HealthDismissalStore, IssueDismissal};
use wealthfolio_core::Result;

pub struct HealthDismissalRepository {
    pool: Arc<DbPool>,
}

impl HealthDismissalRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn get_dismissals_impl(&self) -> Result<Vec<IssueDismissal>> {
        let mut conn = get_connection(&self.pool)?;
        let dismissals_db = wf_health_issue_dismissals
            .select(HealthIssueDismissalDB::as_select())
            .load::<HealthIssueDismissalDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(dismissals_db.into_iter().map(IssueDismissal::from).collect())
    }

    fn get_dismissal_impl(&self, id: &str) -> Result<Option<IssueDismissal>> {
        let mut conn = get_connection(&self.pool)?;
        let result = wf_health_issue_dismissals
            .find(id)
            .select(HealthIssueDismissalDB::as_select())
            .first::<HealthIssueDismissalDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(IssueDismissal::from))
    }
}

#[async_trait]
impl HealthDismissalStore for HealthDismissalRepository {
    async fn save_dismissal(&self, dismissal: &IssueDismissal) -> Result<()> {
        let dismissal_db: HealthIssueDismissalDB = dismissal.clone().into();
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(wf_health_issue_dismissals::table)
            .values(&dismissal_db)
            .on_conflict(issue_id)
            .do_update()
            .set(&dismissal_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn remove_dismissal(&self, id: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(wf_health_issue_dismissals.find(id))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn get_dismissals(&self) -> Result<Vec<IssueDismissal>> {
        self.get_dismissals_impl()
    }

    async fn get_dismissal(&self, id: &str) -> Result<Option<IssueDismissal>> {
        self.get_dismissal_impl(id)
    }

    async fn clear_all(&self) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(wf_health_issue_dismissals::table)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }
}