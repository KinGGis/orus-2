use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::upsert::excluded;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::goals::model::{GoalDB, GoalsAllocationDB, NewGoalDB};
use crate::schema::wf_goals::dsl as goals_dsl;
use crate::schema::wf_goals_allocation;
use wealthfolio_core::goals::{Goal, GoalRepositoryTrait, GoalsAllocation, NewGoal};
use wealthfolio_core::Result;

pub struct GoalRepository {
    pool: Arc<DbPool>,
}

impl GoalRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    pub fn load_goals_impl(&self) -> Result<Vec<Goal>> {
        let mut conn = get_connection(&self.pool)?;
        let goals_db = goals_dsl::wf_goals
            .select(GoalDB::as_select())
            .load::<GoalDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(goals_db.into_iter().map(Goal::from).collect())
    }

    pub fn load_allocations_for_non_achieved_goals_impl(&self) -> Result<Vec<GoalsAllocation>> {
        let mut conn = get_connection(&self.pool)?;
        let allocations_db = wf_goals_allocation::table
            .inner_join(goals_dsl::wf_goals.on(goals_dsl::id.eq(wf_goals_allocation::goal_id)))
            .filter(goals_dsl::is_achieved.eq(false))
            .select(GoalsAllocationDB::as_select())
            .load::<GoalsAllocationDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(allocations_db
            .into_iter()
            .map(GoalsAllocation::from)
            .collect())
    }
}

#[async_trait]
impl GoalRepositoryTrait for GoalRepository {
    fn load_goals(&self) -> Result<Vec<Goal>> {
        self.load_goals_impl()
    }

    async fn insert_new_goal(&self, new_goal: NewGoal) -> Result<Goal> {
        let mut new_goal_db = NewGoalDB::try_from(new_goal)?;
        new_goal_db.id = Some(new_goal_db.id.unwrap_or_else(Uuid::new_v4));
        let mut conn = get_connection(&self.pool)?;
        let result_db = diesel::insert_into(goals_dsl::wf_goals)
            .values(&new_goal_db)
            .returning(GoalDB::as_returning())
            .get_result::<GoalDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(Goal::from(result_db))
    }

    async fn update_goal(&self, goal_update: Goal) -> Result<Goal> {
        let goal_db = GoalDB::try_from(goal_update)?;
        let goal_id = goal_db.id;
        let mut conn = get_connection(&self.pool)?;

        diesel::update(goals_dsl::wf_goals.find(goal_id))
            .set(&goal_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        let result_db = goals_dsl::wf_goals
            .find(goal_id)
            .select(GoalDB::as_select())
            .first::<GoalDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(Goal::from(result_db))
    }

    async fn delete_goal(&self, goal_id_to_delete: String) -> Result<usize> {
        let goal_id = Uuid::parse_str(&goal_id_to_delete).map_err(|err| {
            wealthfolio_core::Error::Validation(wealthfolio_core::errors::ValidationError::InvalidInput(
                format!("Invalid goal_id UUID: {err}"),
            ))
        })?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(goals_dsl::wf_goals.find(goal_id))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn load_allocations_for_non_achieved_goals(&self) -> Result<Vec<GoalsAllocation>> {
        self.load_allocations_for_non_achieved_goals_impl()
    }

    async fn upsert_goal_allocations(&self, allocations: Vec<GoalsAllocation>) -> Result<usize> {
        if allocations.is_empty() {
            return Ok(0);
        }

        let allocation_dbs = allocations
            .into_iter()
            .map(GoalsAllocationDB::try_from)
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;
        let mut affected_rows = 0;

        for allocation_db in allocation_dbs {
            affected_rows += diesel::insert_into(wf_goals_allocation::table)
                .values(&allocation_db)
                .on_conflict(wf_goals_allocation::id)
                .do_update()
                .set((
                    wf_goals_allocation::goal_id.eq(excluded(wf_goals_allocation::goal_id)),
                    wf_goals_allocation::account_id.eq(excluded(wf_goals_allocation::account_id)),
                    wf_goals_allocation::percent_allocation.eq(excluded(wf_goals_allocation::percent_allocation)),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(affected_rows)
    }
}