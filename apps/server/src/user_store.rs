use async_trait::async_trait;
use wealthfolio_core::users::{NewUser, NewUserRole, User, UserRole};

#[async_trait]
pub trait UserStore: Send + Sync {
    fn create_user(&self, new_user: NewUser) -> wealthfolio_core::Result<User>;
    fn get_user_by_id(&self, user_id: &str) -> wealthfolio_core::Result<Option<User>>;
    fn list_users(&self, active_only: bool) -> wealthfolio_core::Result<Vec<User>>;
    fn get_user_roles(&self, user_id: &str) -> wealthfolio_core::Result<Vec<UserRole>>;
    fn assign_role(&self, new_role: NewUserRole) -> wealthfolio_core::Result<UserRole>;
    fn remove_role(&self, user_id: &str, role: &str) -> wealthfolio_core::Result<()>;
}

#[async_trait]
impl UserStore for wealthfolio_storage_sqlite::users::UserRepository {
    fn create_user(&self, new_user: NewUser) -> wealthfolio_core::Result<User> {
        self.create_user(new_user)
    }

    fn get_user_by_id(&self, user_id: &str) -> wealthfolio_core::Result<Option<User>> {
        self.get_user_by_id(user_id)
    }

    fn list_users(&self, active_only: bool) -> wealthfolio_core::Result<Vec<User>> {
        self.list_users(active_only)
    }

    fn get_user_roles(&self, user_id: &str) -> wealthfolio_core::Result<Vec<UserRole>> {
        self.get_user_roles(user_id)
    }

    fn assign_role(&self, new_role: NewUserRole) -> wealthfolio_core::Result<UserRole> {
        self.assign_role(new_role)
    }

    fn remove_role(&self, user_id: &str, role: &str) -> wealthfolio_core::Result<()> {
        self.remove_role(user_id, role)
    }
}

#[async_trait]
impl UserStore for wealthfolio_storage_postgres::users::UserRepository {
    fn create_user(&self, new_user: NewUser) -> wealthfolio_core::Result<User> {
        self.create_user(new_user)
    }

    fn get_user_by_id(&self, user_id: &str) -> wealthfolio_core::Result<Option<User>> {
        self.get_user_by_id(user_id)
    }

    fn list_users(&self, active_only: bool) -> wealthfolio_core::Result<Vec<User>> {
        self.list_users(active_only)
    }

    fn get_user_roles(&self, user_id: &str) -> wealthfolio_core::Result<Vec<UserRole>> {
        self.get_user_roles(user_id)
    }

    fn assign_role(&self, new_role: NewUserRole) -> wealthfolio_core::Result<UserRole> {
        self.assign_role(new_role)
    }

    fn remove_role(&self, user_id: &str, role: &str) -> wealthfolio_core::Result<()> {
        self.remove_role(user_id, role)
    }
}