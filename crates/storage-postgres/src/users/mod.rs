use std::sync::Arc;

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{wf_user_roles, wf_users};
use wealthfolio_core::errors::{Result, ValidationError};
use wealthfolio_core::users::{NewUser, NewUserRole, User, UserRole};
use wealthfolio_core::Error;

#[derive(Queryable, Identifiable, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct UserDB {
    id: Uuid,
    email: String,
    full_name: Option<String>,
    avatar_url: Option<String>,
    preferred_currency: Option<String>,
    is_active: bool,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    last_login_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Queryable, Identifiable, Associations, Insertable, AsChangeset, Selectable, Debug, Clone)]
#[diesel(table_name = wf_user_roles)]
#[diesel(belongs_to(UserDB, foreign_key = user_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct UserRoleDB {
    id: Uuid,
    user_id: Uuid,
    role: String,
    created_at: chrono::DateTime<Utc>,
    created_by: Option<Uuid>,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

impl From<UserDB> for User {
    fn from(db: UserDB) -> Self {
        Self {
            id: db.id.to_string(),
            email: db.email,
            full_name: db.full_name,
            avatar_url: db.avatar_url,
            preferred_currency: db.preferred_currency,
            is_active: db.is_active,
            created_at: db.created_at.to_rfc3339(),
            updated_at: db.updated_at.to_rfc3339(),
            last_login_at: db.last_login_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<UserRoleDB> for UserRole {
    fn from(db: UserRoleDB) -> Self {
        Self {
            id: db.id.to_string(),
            user_id: db.user_id.to_string(),
            role: db.role,
            created_at: db.created_at.to_rfc3339(),
            created_by: db.created_by.map(|value| value.to_string()),
        }
    }
}

pub struct UserRepository {
    pool: Arc<DbPool>,
}

impl UserRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    pub fn create_user(&self, new_user: NewUser) -> Result<User> {
        new_user.validate()?;

        let now = Utc::now();
        let user_db = UserDB {
            id: new_user
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "user_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            email: new_user.email,
            full_name: new_user.full_name,
            avatar_url: new_user.avatar_url,
            preferred_currency: new_user.preferred_currency.or(Some("EUR".to_string())),
            is_active: true,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_users::table)
            .values(&user_db)
            .get_result::<UserDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let parsed_id = parse_uuid(user_id, "user_id")?;
        let mut conn = get_connection(&self.pool)?;

        let result = wf_users::table
            .select(UserDB::as_select())
            .find(parsed_id)
            .first::<UserDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    pub fn list_users(&self, active_only: bool) -> Result<Vec<User>> {
        let mut conn = get_connection(&self.pool)?;
        let mut query = wf_users::table.into_boxed();

        if active_only {
            query = query.filter(wf_users::is_active.eq(true));
        }

        let results = query
            .select(UserDB::as_select())
            .order(wf_users::email.asc())
            .load::<UserDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    pub fn assign_role(&self, new_role: NewUserRole) -> Result<UserRole> {
        new_role.validate()?;

        let role_db = UserRoleDB {
            id: new_role
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "user_role_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            user_id: parse_uuid(&new_role.user_id, "user_id")?,
            role: new_role.role,
            created_at: Utc::now(),
            created_by: new_role
                .created_by
                .as_deref()
                .map(|value| parse_uuid(value, "created_by"))
                .transpose()?,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_user_roles::table)
            .values(&role_db)
            .get_result::<UserRoleDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    pub fn remove_role(&self, user_id: &str, role: &str) -> Result<()> {
        let parsed_user_id = parse_uuid(user_id, "user_id")?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(
            wf_user_roles::table
                .filter(wf_user_roles::user_id.eq(parsed_user_id))
                .filter(wf_user_roles::role.eq(role)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub fn get_user_roles(&self, user_id: &str) -> Result<Vec<UserRole>> {
        let parsed_user_id = parse_uuid(user_id, "user_id")?;
        let mut conn = get_connection(&self.pool)?;

        let results = wf_user_roles::table
            .select(UserRoleDB::as_select())
            .filter(wf_user_roles::user_id.eq(parsed_user_id))
            .load::<UserRoleDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}