//! Storage error types for PostgreSQL.

use thiserror::Error;
use wealthfolio_core::errors::{DatabaseError, Error};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Diesel error: {0}")]
    DieselError(#[from] diesel::result::Error),

    #[error("Pool error: {0}")]
    PoolError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<StorageError> for Error {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::ConnectionError(msg) => Error::Database(DatabaseError::ConnectionFailed(msg)),
            StorageError::QueryError(msg) => Error::Database(DatabaseError::QueryFailed(msg)),
            StorageError::MigrationError(msg) => Error::Database(DatabaseError::MigrationFailed(msg)),
            StorageError::NotFound(msg) => Error::Database(DatabaseError::NotFound(msg)),
            StorageError::DieselError(diesel::result::Error::NotFound) => {
                Error::Database(DatabaseError::NotFound("Record not found".to_string()))
            }
            StorageError::DieselError(e) => Error::Database(DatabaseError::QueryFailed(e.to_string())),
            StorageError::PoolError(msg) => Error::Database(DatabaseError::PoolCreationFailed(msg)),
            StorageError::SerializationError(msg) => Error::Database(DatabaseError::QueryFailed(msg)),
        }
    }
}

impl From<diesel::result::ConnectionError> for StorageError {
    fn from(err: diesel::result::ConnectionError) -> Self {
        StorageError::ConnectionError(err.to_string())
    }
}

impl From<r2d2::Error> for StorageError {
    fn from(err: r2d2::Error) -> Self {
        StorageError::PoolError(err.to_string())
    }
}

/// Trait to convert storage errors to core errors.
pub trait IntoCore<T> {
    fn into_core(self) -> wealthfolio_core::errors::Result<T>;
}

impl<T> IntoCore<T> for Result<T, StorageError> {
    fn into_core(self) -> wealthfolio_core::errors::Result<T> {
        self.map_err(Error::from)
    }
}

impl<T> IntoCore<T> for Result<T, diesel::result::Error> {
    fn into_core(self) -> wealthfolio_core::errors::Result<T> {
        self.map_err(|e| Error::from(StorageError::DieselError(e)))
    }
}

use diesel::r2d2;
