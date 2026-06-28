use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::limits::model::{ContributionLimitDB, NewContributionLimitDB};
use crate::schema::wf_contribution_limits::dsl as limits_dsl;
use wealthfolio_core::limits::{ContributionLimit, ContributionLimitRepositoryTrait, NewContributionLimit};
use wealthfolio_core::Result;

pub struct ContributionLimitRepository {
    pool: Arc<DbPool>,
}

impl ContributionLimitRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn get_contribution_limit_impl(&self, id_param: &str) -> Result<ContributionLimit> {
        let id = Uuid::parse_str(id_param).map_err(|err| {
            wealthfolio_core::Error::Validation(wealthfolio_core::errors::ValidationError::InvalidInput(
                format!("Invalid contribution limit UUID: {err}"),
            ))
        })?;
        let mut conn = get_connection(&self.pool)?;
        let result_db = limits_dsl::wf_contribution_limits
            .find(id)
            .select(ContributionLimitDB::as_select())
            .first::<ContributionLimitDB>(&mut conn)
            .map_err(StorageError::from)?;
        ContributionLimit::try_from(result_db)
    }

    fn get_contribution_limits_impl(&self) -> Result<Vec<ContributionLimit>> {
        let mut conn = get_connection(&self.pool)?;
        let results_db = limits_dsl::wf_contribution_limits
            .select(ContributionLimitDB::as_select())
            .load::<ContributionLimitDB>(&mut conn)
            .map_err(StorageError::from)?;

        results_db
            .into_iter()
            .map(ContributionLimit::try_from)
            .collect()
    }
}

#[async_trait]
impl ContributionLimitRepositoryTrait for ContributionLimitRepository {
    fn get_contribution_limit(&self, id: &str) -> Result<ContributionLimit> {
        self.get_contribution_limit_impl(id)
    }

    fn get_contribution_limits(&self) -> Result<Vec<ContributionLimit>> {
        self.get_contribution_limits_impl()
    }

    async fn create_contribution_limit(
        &self,
        new_limit: NewContributionLimit,
    ) -> Result<ContributionLimit> {
        let mut new_limit_db = NewContributionLimitDB::try_from(new_limit)?;
        new_limit_db.id = Some(new_limit_db.id.unwrap_or_else(Uuid::new_v4));

        let mut conn = get_connection(&self.pool)?;
        let result_db = diesel::insert_into(limits_dsl::wf_contribution_limits)
            .values(&new_limit_db)
            .returning(ContributionLimitDB::as_returning())
            .get_result::<ContributionLimitDB>(&mut conn)
            .map_err(StorageError::from)?;

        ContributionLimit::try_from(result_db)
    }

    async fn update_contribution_limit(
        &self,
        id: &str,
        updated_limit: NewContributionLimit,
    ) -> Result<ContributionLimit> {
        let limit_id = Uuid::parse_str(id).map_err(|err| {
            wealthfolio_core::Error::Validation(wealthfolio_core::errors::ValidationError::InvalidInput(
                format!("Invalid contribution limit UUID: {err}"),
            ))
        })?;
        let updated_limit_db = NewContributionLimitDB::try_from(updated_limit)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::update(limits_dsl::wf_contribution_limits.find(limit_id))
            .set((
                limits_dsl::group_name.eq(updated_limit_db.group_name),
                limits_dsl::contribution_year.eq(updated_limit_db.contribution_year),
                limits_dsl::limit_amount.eq(updated_limit_db.limit_amount),
                limits_dsl::account_ids.eq(updated_limit_db.account_ids),
                limits_dsl::start_date.eq(updated_limit_db.start_date),
                limits_dsl::end_date.eq(updated_limit_db.end_date),
                limits_dsl::updated_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        let result_db = limits_dsl::wf_contribution_limits
            .find(limit_id)
            .select(ContributionLimitDB::as_select())
            .first::<ContributionLimitDB>(&mut conn)
            .map_err(StorageError::from)?;

        ContributionLimit::try_from(result_db)
    }

    async fn delete_contribution_limit(&self, id: &str) -> Result<()> {
        let limit_id = Uuid::parse_str(id).map_err(|err| {
            wealthfolio_core::Error::Validation(wealthfolio_core::errors::ValidationError::InvalidInput(
                format!("Invalid contribution limit UUID: {err}"),
            ))
        })?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(limits_dsl::wf_contribution_limits.find(limit_id))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }
}