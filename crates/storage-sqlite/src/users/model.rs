//! Database models for users and roles.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use wealthfolio_core::users::{NewUser, NewUserRole, User, UserRole};

/// Database model for users table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserDB {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub preferred_currency: Option<String>,
    pub is_active: i32,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

impl From<UserDB> for User {
    fn from(db: UserDB) -> Self {
        Self {
            id: db.id,
            email: db.email,
            full_name: db.full_name,
            avatar_url: db.avatar_url,
            preferred_currency: db.preferred_currency,
            is_active: db.is_active != 0,
            created_at: db.created_at,
            updated_at: db.updated_at,
            last_login_at: db.last_login_at,
        }
    }
}

impl From<NewUser> for UserDB {
    fn from(domain: NewUser) -> Self {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        Self {
            id: domain.id.unwrap_or_default(),
            email: domain.email,
            full_name: domain.full_name,
            avatar_url: domain.avatar_url,
            preferred_currency: domain.preferred_currency.or(Some("EUR".to_string())),
            is_active: 1,
            created_at: now.clone(),
            updated_at: now,
            last_login_at: None,
        }
    }
}

/// Database model for user_roles table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    Associations,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::user_roles)]
#[diesel(belongs_to(UserDB, foreign_key = user_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserRoleDB {
    pub id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: String,
    pub created_by: Option<String>,
}

impl From<UserRoleDB> for UserRole {
    fn from(db: UserRoleDB) -> Self {
        Self {
            id: db.id,
            user_id: db.user_id,
            role: db.role,
            created_at: db.created_at,
            created_by: db.created_by,
        }
    }
}

impl From<NewUserRole> for UserRoleDB {
    fn from(domain: NewUserRole) -> Self {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        Self {
            id: domain.id.unwrap_or_default(),
            user_id: domain.user_id,
            role: domain.role,
            created_at: now,
            created_by: domain.created_by,
        }
    }
}
