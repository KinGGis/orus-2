use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{wf_assets, wf_quotes};
use super::model::AssetDB;
use wealthfolio_core::assets::AlternativeAssetRepositoryTrait;
use wealthfolio_core::errors::{DatabaseError, ValidationError};
use wealthfolio_core::{Error, Result};

pub struct AlternativeAssetRepository {
    pool: Arc<DbPool>,
}

impl AlternativeAssetRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn parse_asset_id(asset_id: &str) -> Result<Uuid> {
        Uuid::parse_str(asset_id).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid asset_id UUID: {err}"
            )))
        })
    }
}

#[async_trait]
impl AlternativeAssetRepositoryTrait for AlternativeAssetRepository {
    async fn delete_alternative_asset(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;

        let linked_assets = wf_assets::table
            .filter(wf_assets::metadata.is_not_null())
            .select(AssetDB::as_select())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        for liability in linked_assets.into_iter().filter(|asset| {
            asset.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("linked_asset_id"))
                .and_then(|value| value.as_str())
                == Some(asset_id)
        }) {
            if let Some(mut metadata_json) = liability.metadata.clone() {
                if let Some(obj) = metadata_json.as_object_mut() {
                    obj.remove("linked_asset_id");
                }

                diesel::update(wf_assets::table.filter(wf_assets::id.eq(liability.id)))
                    .set(wf_assets::metadata.eq(Some(metadata_json)))
                    .execute(&mut conn)
                    .map_err(StorageError::from)?;
            }
        }

        diesel::delete(
            wf_quotes::table
                .filter(wf_quotes::asset_id.eq(parsed_asset_id))
                .filter(wf_quotes::source.eq("MANUAL")),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        let deleted = diesel::delete(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        if deleted == 0 {
            return Err(Error::Database(DatabaseError::NotFound(format!(
                "Alternative asset not found: {asset_id}"
            ))));
        }

        Ok(())
    }

    async fn update_asset_metadata(
        &self,
        asset_id: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        self.update_asset_details(asset_id, None, None, metadata, None)
            .await
    }

    fn find_liabilities_linked_to(&self, linked_asset_id: &str) -> Result<Vec<String>> {
        let mut conn = get_connection(&self.pool)?;
        let assets = wf_assets::table
            .filter(wf_assets::metadata.is_not_null())
            .select(AssetDB::as_select())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(assets
            .into_iter()
            .filter(|asset| {
                asset.metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("linked_asset_id"))
                    .and_then(|value| value.as_str())
                    == Some(linked_asset_id)
            })
            .map(|asset| asset.id.to_string())
            .collect())
    }

    async fn update_asset_details(
        &self,
        asset_id: &str,
        name: Option<&str>,
        display_code: Option<&str>,
        metadata: Option<serde_json::Value>,
        notes: Option<&str>,
    ) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;

        let updated = diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                name.map(|value| wf_assets::name.eq(value)),
                display_code.map(|value| wf_assets::display_code.eq(value)),
                Some(wf_assets::metadata.eq(metadata)),
                notes.map(|value| wf_assets::notes.eq(Some(value.to_string()))),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        if updated == 0 {
            return Err(Error::Database(DatabaseError::NotFound(format!(
                "Asset not found: {asset_id}"
            ))));
        }

        Ok(())
    }
}