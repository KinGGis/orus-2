//! Database connection and pool management for PostgreSQL/Supabase.

use log::{error, info};
use std::sync::Arc;

use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager, Pool, PooledConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use wealthfolio_core::errors::{DatabaseError, Error, Result};

use crate::errors::StorageError;

// Embed migrations from the migrations folder
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = PooledConnection<ConnectionManager<PgConnection>>;

/// Create a connection pool to PostgreSQL/Supabase.
///
/// The database URL should be in the format:
/// `postgresql://user:password@host:port/database`
///
/// For Supabase, use the connection string from the dashboard:
/// `postgresql://postgres.[ref]:[password]@aws-0-[region].pooler.supabase.com:6543/postgres`
pub fn create_pool(database_url: &str) -> Result<Arc<DbPool>> {
    info!("Creating PostgreSQL connection pool");
    
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(10)
        .min_idle(Some(2))
        .connection_timeout(std::time::Duration::from_secs(30))
        .build(manager)
        .map_err(|e| {
            error!("Failed to create PostgreSQL pool: {}", e);
            DatabaseError::PoolCreationFailed(e.to_string())
        })?;
    
    info!("PostgreSQL connection pool created successfully");
    Ok(Arc::new(pool))
}

/// Get a connection from the pool.
pub fn get_connection(pool: &DbPool) -> Result<DbConnection> {
    pool.get().map_err(|e| {
        error!("Failed to get connection from pool: {}", e);
        Error::Database(DatabaseError::ConnectionFailed(e.to_string()))
    })
}

/// Run pending database migrations.
///
/// Note: For Supabase, migrations should be run via the Supabase dashboard
/// or CLI. This function is mainly for local development.
pub fn run_migrations(database_url: &str) -> Result<()> {
    info!("Running PostgreSQL database migrations");
    
    use diesel::Connection;
    let mut connection = PgConnection::establish(database_url).map_err(|e| {
        error!("Failed to establish connection for migrations: {}", e);
        StorageError::ConnectionError(e.to_string())
    })?;

    let migration_result: Result<Vec<String>> = connection
        .run_pending_migrations(MIGRATIONS)
        .map(|versions| {
            versions
                .into_iter()
                .map(|version| version.to_string())
                .collect()
        })
        .map_err(|e| {
            error!("Database migration failed: {}", e);
            Error::Database(DatabaseError::MigrationFailed(e.to_string()))
        });

    let result = migration_result?;

    if result.is_empty() {
        info!("No pending migrations to apply.");
    } else {
        info!("Applied the following migrations:");
        for migration_version in &result {
            info!("  - {}", migration_version);
        }
    }

    Ok(())
}

/// Get the database URL from environment or config.
///
/// Priority:
/// 1. `DATABASE_URL` environment variable
/// 2. `SUPABASE_DB_URL` environment variable
/// 3. Default local PostgreSQL
pub fn get_database_url() -> String {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    
    if let Ok(url) = std::env::var("SUPABASE_DB_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    
    // Default for local development
    "postgresql://postgres:postgres@localhost:5432/wealthfolio".to_string()
}
