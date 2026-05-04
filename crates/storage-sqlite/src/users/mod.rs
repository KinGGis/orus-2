//! SQLite storage implementation for users and RBAC.

mod model;
mod repository;

pub use model::{UserDB, UserRoleDB};
pub use repository::UserRepository;
