//! Repository implementation for users and roles.

use diesel::prelude::*;
use diesel::r2d2::{self, Pool};
use diesel::sqlite::SqliteConnection;
use std::sync::Arc;

use crate::db::get_connection;
use crate::errors::StorageError;
use crate::schema::{user_roles, users};

use super::model::{UserDB, UserRoleDB};
use wealthfolio_core::errors::Result;
use wealthfolio_core::users::{NewUser, NewUserRole, User, UserRole};

/// Repository for managing users and roles in the database
pub struct UserRepository {
    pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>,
}

impl UserRepository {
    /// Creates a new UserRepository instance
    pub fn new(pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>) -> Self {
        Self { pool }
    }

    /// Creates a new user
    pub fn create_user(&self, new_user: NewUser) -> Result<User> {
        new_user.validate()?;

        let mut conn = get_connection(&self.pool)?;

        let mut user_db: UserDB = new_user.into();
        user_db.id = uuid::Uuid::new_v4().to_string();

        diesel::insert_into(users::table)
            .values(&user_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(user_db.into())
    }

    /// Retrieves a user by ID
    pub fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let mut conn = get_connection(&self.pool)?;

        let result = users::table
            .select(UserDB::as_select())
            .find(user_id)
            .first::<UserDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    /// Retrieves a user by email
    pub fn get_user_by_email(&self, email_addr: &str) -> Result<Option<User>> {
        let mut conn = get_connection(&self.pool)?;

        let result = users::table
            .select(UserDB::as_select())
            .filter(users::email.eq(email_addr))
            .first::<UserDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Into::into))
    }

    /// Lists all active users
    pub fn list_users(&self, active_only: bool) -> Result<Vec<User>> {
        let mut conn = get_connection(&self.pool)?;

        let mut query = users::table.into_boxed();

        if active_only {
            query = query.filter(users::is_active.eq(1));
        }

        let results = query
            .select(UserDB::as_select())
            .order(users::email.asc())
            .load::<UserDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Updates the last login timestamp for a user
    pub fn update_last_login(&self, user_id: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        diesel::update(users::table.find(user_id))
            .set((
                users::last_login_at.eq(Some(&now)),
                users::updated_at.eq(&now),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    /// Assigns a role to a user
    pub fn assign_role(&self, new_role: NewUserRole) -> Result<UserRole> {
        new_role.validate()?;

        let mut conn = get_connection(&self.pool)?;

        let mut role_db: UserRoleDB = new_role.into();
        role_db.id = uuid::Uuid::new_v4().to_string();

        diesel::insert_into(user_roles::table)
            .values(&role_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(role_db.into())
    }

    /// Removes a role from a user
    pub fn remove_role(&self, user_id: &str, role: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(
            user_roles::table
                .filter(user_roles::user_id.eq(user_id))
                .filter(user_roles::role.eq(role)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        Ok(())
    }

    /// Gets all roles for a user
    pub fn get_user_roles(&self, user_id: &str) -> Result<Vec<UserRole>> {
        let mut conn = get_connection(&self.pool)?;

        let results = user_roles::table
            .select(UserRoleDB::as_select())
            .filter(user_roles::user_id.eq(user_id))
            .load::<UserRoleDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Checks if a user has a specific role
    pub fn user_has_role(&self, user_id: &str, role: &str) -> Result<bool> {
        let mut conn = get_connection(&self.pool)?;

        let count: i64 = user_roles::table
            .filter(user_roles::user_id.eq(user_id))
            .filter(user_roles::role.eq(role))
            .count()
            .get_result(&mut conn)
            .map_err(StorageError::from)?;

        Ok(count > 0)
    }

    /// Gets all users with a specific role
    pub fn get_users_by_role(&self, role: &str) -> Result<Vec<User>> {
        let mut conn = get_connection(&self.pool)?;

        let user_ids: Vec<String> = user_roles::table
            .select(user_roles::user_id)
            .filter(user_roles::role.eq(role))
            .load::<String>(&mut conn)
            .map_err(StorageError::from)?;

        let results = users::table
            .select(UserDB::as_select())
            .filter(users::id.eq_any(user_ids))
            .load::<UserDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Into::into).collect())
    }
}
