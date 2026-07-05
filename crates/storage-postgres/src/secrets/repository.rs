use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use diesel::prelude::*;
use rand::{rngs::OsRng, RngCore};

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_secrets::dsl as secrets_dsl;
use crate::secrets::model::NewSecretDB;
use wealthfolio_core::errors::{Error, Result};
use wealthfolio_core::secrets::{format_service_id, SecretStore};

/// Marker prefix identifying an encrypted secret value stored in Postgres.
const ENC_PREFIX: &str = "enc:v1:";

/// PostgreSQL-backed [`SecretStore`].
///
/// Secrets are persisted in the `wf_secrets` table so they survive redeployments
/// even when the server has no persistent disk. When `encryption_key` is set,
/// values are encrypted at rest with ChaCha20-Poly1305 (same key derivation as
/// the file-based store); otherwise they are stored as plaintext.
pub struct PostgresSecretStore {
    pool: Arc<DbPool>,
    encryption_key: Option<[u8; 32]>,
}

impl PostgresSecretStore {
    pub fn new(pool: Arc<DbPool>, encryption_key: Option<[u8; 32]>) -> Self {
        Self {
            pool,
            encryption_key,
        }
    }

    fn encrypt(&self, plaintext: &str) -> Result<String> {
        match self.encryption_key {
            Some(key) => {
                let mut nonce_bytes = [0u8; 12];
                OsRng.fill_bytes(&mut nonce_bytes);
                let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
                let nonce = Nonce::from_slice(&nonce_bytes);
                let ciphertext = cipher
                    .encrypt(nonce, plaintext.as_bytes())
                    .map_err(|_| Error::Secret("Failed to encrypt secret".into()))?;
                Ok(format!(
                    "{}{}:{}",
                    ENC_PREFIX,
                    BASE64.encode(nonce_bytes),
                    BASE64.encode(ciphertext)
                ))
            }
            None => Ok(plaintext.to_string()),
        }
    }

    fn decrypt(&self, stored: &str) -> Result<String> {
        let Some(payload) = stored.strip_prefix(ENC_PREFIX) else {
            // Value was stored in plaintext.
            return Ok(stored.to_string());
        };

        let key = self.encryption_key.ok_or_else(|| {
            Error::Secret("WF_SECRET_KEY must be set to decrypt stored secrets".into())
        })?;

        let (nonce_b64, cipher_b64) = payload
            .split_once(':')
            .ok_or_else(|| Error::Secret("Malformed encrypted secret".into()))?;
        let nonce_bytes = BASE64
            .decode(nonce_b64)
            .map_err(|e| Error::Secret(format!("Failed to decode nonce: {e}")))?;
        let cipher_bytes = BASE64
            .decode(cipher_b64)
            .map_err(|e| Error::Secret(format!("Failed to decode ciphertext: {e}")))?;

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, cipher_bytes.as_ref())
            .map_err(|_| Error::Secret("Failed to decrypt secret".into()))?;
        String::from_utf8(plaintext)
            .map_err(|e| Error::Secret(format!("Decrypted secret is not valid UTF-8: {e}")))
    }
}

impl SecretStore for PostgresSecretStore {
    fn set_secret(&self, service: &str, secret: &str) -> Result<()> {
        let key = format_service_id(service);
        let value = self.encrypt(secret)?;
        let mut conn = get_connection(&self.pool)?;

        let record = NewSecretDB {
            secret_key: key,
            secret_value: value.clone(),
        };

        diesel::insert_into(secrets_dsl::wf_secrets)
            .values(&record)
            .on_conflict(secrets_dsl::secret_key)
            .do_update()
            .set((
                secrets_dsl::secret_value.eq(value),
                secrets_dsl::updated_at.eq(diesel::dsl::now),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn get_secret(&self, service: &str) -> Result<Option<String>> {
        let key = format_service_id(service);
        let mut conn = get_connection(&self.pool)?;

        let stored: Option<String> = secrets_dsl::wf_secrets
            .filter(secrets_dsl::secret_key.eq(&key))
            .select(secrets_dsl::secret_value)
            .first::<String>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        match stored {
            Some(value) => Ok(Some(self.decrypt(&value)?)),
            None => Ok(None),
        }
    }

    fn delete_secret(&self, service: &str) -> Result<()> {
        let key = format_service_id(service);
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(secrets_dsl::wf_secrets.filter(secrets_dsl::secret_key.eq(&key)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }
}
