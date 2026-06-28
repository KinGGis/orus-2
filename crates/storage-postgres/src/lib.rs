//! PostgreSQL/Supabase storage implementation for Wealthfolio.
//!
//! This crate provides database-related functionality using Diesel ORM with PostgreSQL.
//! It implements the repository traits defined in `wealthfolio-core` and is designed
//! to work with Supabase PostgreSQL.
//!
//! # Architecture
//!
//! This crate mirrors `storage-sqlite` but uses PostgreSQL instead of SQLite.
//! Tables are prefixed with `wf_` to coexist with Orus tables in Supabase.
//!
//! ```text
//! core (domain)          connect (sync)
//!       │                      │
//!       └──────────┬───────────┘
//!                  │
//!                  ▼
//!          storage-postgres (this crate)
//!                  │
//!                  ▼
//!          Supabase PostgreSQL
//! ```

pub mod accounts;
pub mod activities;
pub mod ai_chat;
pub mod assets;
pub mod db;
pub mod errors;
pub mod fx;
pub mod goals;
pub mod health;
pub mod limits;
pub mod market_data;
pub mod portfolio;
pub mod revolut;
pub mod schema;
pub mod settings;
pub mod sync;
pub mod system_accounts;
pub mod taxonomies;
pub mod users;

// Repository implementations (to be added)
// pub mod accounts;
// pub mod activities;
// pub mod fx;
// etc.

// Re-export database utilities
pub use db::{create_pool, get_connection, run_migrations, DbConnection, DbPool};

// Re-export storage errors
pub use errors::StorageError;

// Re-export from wealthfolio-core for convenience
pub use wealthfolio_core::errors::{DatabaseError, Error, Result};
