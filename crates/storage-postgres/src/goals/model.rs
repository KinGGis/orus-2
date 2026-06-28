use bigdecimal::BigDecimal;
use diesel::prelude::*;
use std::str::FromStr;
use uuid::Uuid;

use crate::system_accounts::{account_id_to_domain, parse_account_id};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::goals::{Goal, GoalsAllocation, NewGoal};
use wealthfolio_core::{Error, Result};

#[derive(
    Queryable,
    Identifiable,
    AsChangeset,
    Selectable,
    PartialEq,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::wf_goals)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GoalDB {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub target_amount: BigDecimal,
    pub is_achieved: bool,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_goals)]
pub struct NewGoalDB {
    pub id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_amount: BigDecimal,
    pub is_achieved: bool,
}

#[derive(
    Insertable,
    Queryable,
    Identifiable,
    Associations,
    AsChangeset,
    Selectable,
    PartialEq,
    Debug,
    Clone,
)]
#[diesel(belongs_to(GoalDB, foreign_key = goal_id))]
#[diesel(table_name = crate::schema::wf_goals_allocation)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GoalsAllocationDB {
    pub id: Uuid,
    pub goal_id: Uuid,
    pub account_id: Uuid,
    pub percent_allocation: i32,
}

fn bigdecimal_from_f64(value: f64, field: &str) -> Result<BigDecimal> {
    BigDecimal::from_str(&value.to_string()).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} decimal: {err}"
        )))
    })
}

fn f64_from_bigdecimal(value: &BigDecimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or_default()
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

impl From<GoalDB> for Goal {
    fn from(db: GoalDB) -> Self {
        Self {
            id: db.id.to_string(),
            title: db.title,
            description: db.description,
            target_amount: f64_from_bigdecimal(&db.target_amount),
            is_achieved: db.is_achieved,
        }
    }
}

impl From<GoalsAllocationDB> for GoalsAllocation {
    fn from(db: GoalsAllocationDB) -> Self {
        Self {
            id: db.id.to_string(),
            goal_id: db.goal_id.to_string(),
            account_id: account_id_to_domain(db.account_id),
            percent_allocation: db.percent_allocation,
        }
    }
}

impl TryFrom<NewGoal> for NewGoalDB {
    type Error = Error;

    fn try_from(domain: NewGoal) -> Result<Self> {
        Ok(Self {
            id: domain.id.as_deref().map(|value| parse_uuid(value, "goal_id")).transpose()?,
            title: domain.title,
            description: domain.description,
            target_amount: bigdecimal_from_f64(domain.target_amount, "target_amount")?,
            is_achieved: domain.is_achieved,
        })
    }
}

impl TryFrom<GoalsAllocation> for GoalsAllocationDB {
    type Error = Error;

    fn try_from(domain: GoalsAllocation) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&domain.id, "goal_allocation_id")?,
            goal_id: parse_uuid(&domain.goal_id, "goal_id")?,
            account_id: parse_account_id(&domain.account_id)?,
            percent_allocation: domain.percent_allocation,
        })
    }
}

impl TryFrom<Goal> for GoalDB {
    type Error = Error;

    fn try_from(domain: Goal) -> Result<Self> {
        Ok(Self {
            id: parse_uuid(&domain.id, "goal_id")?,
            title: domain.title,
            description: domain.description,
            target_amount: bigdecimal_from_f64(domain.target_amount, "target_amount")?,
            is_achieved: domain.is_achieved,
        })
    }
}