use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use diesel::dsl::count_star;
use diesel::prelude::*;
use uuid::Uuid;

use super::model::{AssetDB, InsertableAssetDB};
use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{wf_activities, wf_assets, wf_quotes};
use wealthfolio_core::assets::{Asset, AssetRepositoryTrait, NewAsset, UpdateAssetProfile};
use wealthfolio_core::errors::{Result, ValidationError};
use wealthfolio_core::Error;

pub struct AssetRepository {
    pool: Arc<DbPool>,
}

impl AssetRepository {
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

    pub fn get_by_id_impl(&self, asset_id: &str) -> Result<Asset> {
        let parsed_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result = wf_assets::table
            .select(AssetDB::as_select())
            .find(parsed_id)
            .first::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(Asset::from(result))
    }

    pub fn list_impl(&self) -> Result<Vec<Asset>> {
        let mut conn = get_connection(&self.pool)?;
        let results = wf_assets::table
            .select(AssetDB::as_select())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Asset::from).collect())
    }

    pub fn list_by_asset_ids_impl(&self, asset_ids: &[String]) -> Result<Vec<Asset>> {
        if asset_ids.is_empty() {
            return Ok(Vec::new());
        }

        let parsed_ids = asset_ids
            .iter()
            .map(|value| Self::parse_asset_id(value))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;
        let results = wf_assets::table
            .filter(wf_assets::id.eq_any(parsed_ids))
            .select(AssetDB::as_select())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Asset::from).collect())
    }

    pub fn search_by_symbol_impl(&self, query: &str) -> Result<Vec<Asset>> {
        let mut conn = get_connection(&self.pool)?;
        let pattern = format!("%{}%", query.to_uppercase());
        let results = wf_assets::table
            .select(AssetDB::as_select())
            .filter(
                wf_assets::display_code
                    .ilike(&pattern)
                    .or(wf_assets::instrument_symbol.ilike(&pattern))
                    .or(wf_assets::name.ilike(&pattern)),
            )
            .order(wf_assets::display_code.asc())
            .limit(50)
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Asset::from).collect())
    }
}

#[async_trait]
impl AssetRepositoryTrait for AssetRepository {
    async fn create(&self, new_asset: NewAsset) -> Result<Asset> {
        new_asset.validate()?;
        let asset_db: InsertableAssetDB = new_asset.into();
        let mut conn = get_connection(&self.pool)?;
        let result_db = diesel::insert_into(wf_assets::table)
            .values(&asset_db)
            .get_result::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result_db.into())
    }

    async fn create_batch(&self, new_assets: Vec<NewAsset>) -> Result<Vec<Asset>> {
        if new_assets.is_empty() {
            return Ok(Vec::new());
        }
        for asset in &new_assets {
            asset.validate()?;
        }

        let assets_db: Vec<InsertableAssetDB> = new_assets.into_iter().map(Into::into).collect();
        let ids: Vec<Uuid> = assets_db.iter().map(|asset| asset.id).collect();
        let mut conn = get_connection(&self.pool)?;
        let existing_ids: HashSet<Uuid> = wf_assets::table
            .filter(wf_assets::id.eq_any(&ids))
            .select(wf_assets::id)
            .load::<Uuid>(&mut conn)
            .map_err(StorageError::from)?
            .into_iter()
            .collect();

        for asset_db in &assets_db {
            diesel::insert_into(wf_assets::table)
                .values(asset_db)
                .on_conflict(wf_assets::id)
                .do_nothing()
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        let results = wf_assets::table
            .filter(wf_assets::id.eq_any(&ids))
            .select(AssetDB::as_select())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        let _newly_created = results
            .iter()
            .filter(|asset| !existing_ids.contains(&asset.id))
            .count();

        Ok(results.into_iter().map(Asset::from).collect())
    }

    async fn update_profile(&self, asset_id: &str, payload: UpdateAssetProfile) -> Result<Asset> {
        payload.validate()?;
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let existing: AssetDB = wf_assets::table
            .filter(wf_assets::id.eq(parsed_asset_id))
            .select(AssetDB::as_select())
            .first(&mut conn)
            .map_err(StorageError::from)?;

        let result_db = diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                wf_assets::name.eq(payload.name),
                wf_assets::kind.eq(
                    payload
                        .kind
                        .as_ref()
                        .map(|value| value.as_db_str().to_string())
                        .unwrap_or(existing.kind),
                ),
                wf_assets::display_code.eq(payload.display_code),
                wf_assets::notes.eq(Some(payload.notes)),
                wf_assets::metadata.eq(payload.metadata.or(existing.metadata)),
                wf_assets::quote_mode.eq(
                    payload
                        .quote_mode
                        .map(|value| value.as_db_str().to_string())
                        .unwrap_or(existing.quote_mode),
                ),
                wf_assets::quote_ccy.eq(payload.quote_ccy.unwrap_or(existing.quote_ccy)),
                wf_assets::instrument_type.eq(
                    if payload.instrument_type.is_some() {
                        payload
                            .instrument_type
                            .map(|value| value.as_db_str().to_string())
                    } else {
                        existing.instrument_type
                    },
                ),
                wf_assets::instrument_symbol.eq(
                    payload.instrument_symbol.or(existing.instrument_symbol),
                ),
                wf_assets::instrument_exchange_mic.eq(
                    payload
                        .instrument_exchange_mic
                        .or(existing.instrument_exchange_mic),
                ),
                wf_assets::provider_config.eq(payload.provider_config.or(existing.provider_config)),
                wf_assets::updated_at.eq(chrono::Utc::now()),
            ))
            .get_result::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result_db.into())
    }

    async fn update_quote_mode(&self, asset_id: &str, quote_mode: &str) -> Result<Asset> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result_db = diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                wf_assets::quote_mode.eq(quote_mode.to_string()),
                wf_assets::updated_at.eq(chrono::Utc::now()),
            ))
            .get_result::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result_db.into())
    }

    fn get_by_id(&self, asset_id: &str) -> Result<Asset> {
        self.get_by_id_impl(asset_id)
    }

    fn list(&self) -> Result<Vec<Asset>> {
        self.list_impl()
    }

    fn list_by_asset_ids(&self, asset_ids: &[String]) -> Result<Vec<Asset>> {
        self.list_by_asset_ids_impl(asset_ids)
    }

    async fn delete(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let activity_count: i64 = wf_activities::table
            .filter(wf_activities::asset_id.eq(parsed_asset_id))
            .select(count_star())
            .first(&mut conn)
            .map_err(StorageError::from)?;

        if activity_count > 0 {
            return Err(Error::ConstraintViolation(
                "Cannot delete asset: it has existing activities. Please delete all associated activities first.".to_string(),
            ));
        }

        diesel::delete(wf_quotes::table.filter(wf_quotes::asset_id.eq(parsed_asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        diesel::delete(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn search_by_symbol(&self, query: &str) -> Result<Vec<Asset>> {
        self.search_by_symbol_impl(query)
    }

    fn find_by_instrument_key(&self, instrument_key: &str) -> Result<Option<Asset>> {
        let mut conn = get_connection(&self.pool)?;
        let result = wf_assets::table
            .filter(wf_assets::instrument_key.eq(instrument_key))
            .select(AssetDB::as_select())
            .first::<AssetDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Asset::from))
    }

    async fn cleanup_legacy_metadata(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let existing: AssetDB = wf_assets::table
            .filter(wf_assets::id.eq(parsed_asset_id))
            .select(AssetDB::as_select())
            .first(&mut conn)
            .map_err(StorageError::from)?;
        let new_metadata = existing.metadata.and_then(|meta| {
            meta.get("identifiers")
                .cloned()
                .map(|ids| serde_json::json!({ "identifiers": ids }))
        });

        diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                wf_assets::metadata.eq(new_metadata),
                wf_assets::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn deactivate(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                wf_assets::is_active.eq(false),
                wf_assets::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn reactivate(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = Self::parse_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_asset_id)))
            .set((
                wf_assets::is_active.eq(true),
                wf_assets::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn copy_user_metadata(&self, source_id: &str, target_id: &str) -> Result<()> {
        let parsed_source_id = Self::parse_asset_id(source_id)?;
        let parsed_target_id = Self::parse_asset_id(target_id)?;
        let mut conn = get_connection(&self.pool)?;
        let source: AssetDB = wf_assets::table
            .filter(wf_assets::id.eq(parsed_source_id))
            .select(AssetDB::as_select())
            .first(&mut conn)
            .map_err(StorageError::from)?;

        if let Some(notes) = source.notes.filter(|value| !value.trim().is_empty()) {
            diesel::update(wf_assets::table.filter(wf_assets::id.eq(parsed_target_id)))
                .set((
                    wf_assets::notes.eq(notes),
                    wf_assets::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(())
    }

    async fn deactivate_orphaned_investments(&self) -> Result<Vec<String>> {
        #[derive(QueryableByName)]
        struct OrphanAssetRow {
            #[diesel(sql_type = diesel::sql_types::Uuid)]
            id: Uuid,
        }

        let mut conn = get_connection(&self.pool)?;
        let orphan_rows: Vec<OrphanAssetRow> = diesel::sql_query(
            "SELECT id FROM wf_assets WHERE kind = 'INVESTMENT' AND is_active = true AND id NOT IN (SELECT DISTINCT asset_id FROM wf_activities WHERE asset_id IS NOT NULL)",
        )
        .load(&mut conn)
        .map_err(StorageError::from)?;
        let orphan_ids: Vec<Uuid> = orphan_rows.into_iter().map(|row| row.id).collect();

        if !orphan_ids.is_empty() {
            diesel::update(wf_assets::table.filter(wf_assets::id.eq_any(&orphan_ids)))
                .set((
                    wf_assets::is_active.eq(false),
                    wf_assets::updated_at.eq(chrono::Utc::now()),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(orphan_ids.into_iter().map(|id| id.to_string()).collect())
    }
}