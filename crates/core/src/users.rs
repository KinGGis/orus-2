//! Users and RBAC module - domain models for multi-user support.
//!
//! This module provides the foundation for DFC's role-based access control.
//! Roles follow a hierarchy: superadmin > admin > portfolio_manager > trader > analyst > viewer

use serde::{Deserialize, Serialize};

/// Available roles in the DFC system.
/// Role permissions are enforced at the API layer (Axum middleware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full system access, can manage users and roles
    Superadmin,
    /// Administrative access, can configure system settings
    Admin,
    /// Can manage portfolios, trigger reports, view all data
    PortfolioManager,
    /// Can execute trades, view positions
    Trader,
    /// Read-only access to analytics and reports
    Analyst,
    /// Basic read-only access
    Viewer,
}

impl Role {
    /// Returns the string representation used in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Superadmin => "superadmin",
            Role::Admin => "admin",
            Role::PortfolioManager => "portfolio_manager",
            Role::Trader => "trader",
            Role::Analyst => "analyst",
            Role::Viewer => "viewer",
        }
    }

    /// Parse a role from its database string representation.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "superadmin" => Some(Role::Superadmin),
            "admin" => Some(Role::Admin),
            "portfolio_manager" => Some(Role::PortfolioManager),
            "trader" => Some(Role::Trader),
            "analyst" => Some(Role::Analyst),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }

    /// Returns the role hierarchy level (higher = more permissions).
    pub fn level(&self) -> u8 {
        match self {
            Role::Superadmin => 100,
            Role::Admin => 80,
            Role::PortfolioManager => 60,
            Role::Trader => 40,
            Role::Analyst => 20,
            Role::Viewer => 10,
        }
    }

    /// Check if this role has at least the permissions of another role.
    pub fn has_permission_of(&self, other: &Role) -> bool {
        self.level() >= other.level()
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Domain model representing a user in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub preferred_currency: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            id: String::new(),
            email: String::new(),
            full_name: None,
            avatar_url: None,
            preferred_currency: Some("EUR".to_string()),
            is_active: true,
            created_at: String::new(),
            updated_at: String::new(),
            last_login_at: None,
        }
    }
}

/// Input model for creating a new user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub email: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub preferred_currency: Option<String>,
}

impl NewUser {
    /// Validates the new user data.
    pub fn validate(&self) -> crate::Result<()> {
        use crate::{errors::ValidationError, Error};

        if self.email.trim().is_empty() {
            return Err(Error::Validation(ValidationError::InvalidInput(
                "Email cannot be empty".to_string(),
            )));
        }
        if !self.email.contains('@') {
            return Err(Error::Validation(ValidationError::InvalidInput(
                "Invalid email format".to_string(),
            )));
        }
        Ok(())
    }
}

/// Domain model representing a user-role assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRole {
    pub id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: String,
    pub created_by: Option<String>,
}

impl UserRole {
    /// Parse the role string into a Role enum.
    pub fn role_enum(&self) -> Option<Role> {
        Role::from_str(&self.role)
    }
}

/// Input model for assigning a role to a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUserRole {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub user_id: String,
    pub role: String,
    pub created_by: Option<String>,
}

impl NewUserRole {
    /// Validates the new user role data.
    pub fn validate(&self) -> crate::Result<()> {
        use crate::{errors::ValidationError, Error};

        if self.user_id.trim().is_empty() {
            return Err(Error::Validation(ValidationError::InvalidInput(
                "User ID cannot be empty".to_string(),
            )));
        }
        if Role::from_str(&self.role).is_none() {
            return Err(Error::Validation(ValidationError::InvalidInput(
                format!("Invalid role: {}. Must be one of: superadmin, admin, portfolio_manager, trader, analyst, viewer", self.role),
            )));
        }
        Ok(())
    }
}

/// A user with their assigned roles (for API responses).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserWithRoles {
    #[serde(flatten)]
    pub user: User,
    pub roles: Vec<Role>,
}

impl UserWithRoles {
    /// Check if the user has a specific role.
    pub fn has_role(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }

    /// Check if the user has any of the specified roles.
    pub fn has_any_role(&self, roles: &[Role]) -> bool {
        roles.iter().any(|r| self.roles.contains(r))
    }

    /// Check if the user has permission equivalent to or higher than the given role.
    pub fn has_permission_of(&self, role: Role) -> bool {
        self.roles.iter().any(|r| r.has_permission_of(&role))
    }

    /// Get the highest role level this user has.
    pub fn highest_role_level(&self) -> u8 {
        self.roles.iter().map(|r| r.level()).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        assert!(Role::Superadmin.has_permission_of(&Role::Admin));
        assert!(Role::Admin.has_permission_of(&Role::PortfolioManager));
        assert!(Role::PortfolioManager.has_permission_of(&Role::Viewer));
        assert!(!Role::Viewer.has_permission_of(&Role::Admin));
    }

    #[test]
    fn test_role_parsing() {
        assert_eq!(Role::from_str("superadmin"), Some(Role::Superadmin));
        assert_eq!(Role::from_str("portfolio_manager"), Some(Role::PortfolioManager));
        assert_eq!(Role::from_str("invalid"), None);
    }

    #[test]
    fn test_new_user_validation() {
        let valid = NewUser {
            id: None,
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            avatar_url: None,
            preferred_currency: None,
        };
        assert!(valid.validate().is_ok());

        let invalid = NewUser {
            id: None,
            email: "not-an-email".to_string(),
            full_name: None,
            avatar_url: None,
            preferred_currency: None,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_new_user_role_validation() {
        let valid = NewUserRole {
            id: None,
            user_id: "user-123".to_string(),
            role: "analyst".to_string(),
            created_by: None,
        };
        assert!(valid.validate().is_ok());

        let invalid = NewUserRole {
            id: None,
            user_id: "user-123".to_string(),
            role: "manager".to_string(), // Invalid role
            created_by: None,
        };
        assert!(invalid.validate().is_err());
    }
}
