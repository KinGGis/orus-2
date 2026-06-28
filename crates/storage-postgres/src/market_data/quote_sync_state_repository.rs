use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Nullable, Text, Timestamptz};
use log::debug;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::market_data::model::{
    parse_quote_sync_asset_id, QuoteSyncStateDB, QuoteSyncStateUpdateDB,
};
use crate::schema::wf_quote_sync_state::dsl as qss_dsl;
use wealthfolio_core::quotes::{ProviderSyncStats, QuoteSyncState, SyncStateStore};
use wealthfolio_core::Result;

pub struct QuoteSyncStateRepository {
    pool: Arc<DbPool>,
}

impl QuoteSyncStateRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SyncStateStore for QuoteSyncStateRepository {
    fn get_provider_sync_stats(&self) -> Result<Vec<ProviderSyncStats>> {
        #[derive(QueryableByName)]
        struct ProviderSyncStatsRow {
            #[diesel(sql_type = Text)]
            provider_id: String,
            #[diesel(sql_type = BigInt)]
            asset_count: i64,
            #[diesel(sql_type = BigInt)]
            error_count: i64,
            #[diesel(sql_type = Nullable<Timestamptz>)]
            last_synced_at: Option<DateTime<Utc>>,
            #[diesel(sql_type = Nullable<Text>)]
            last_error: Option<String>,
            #[diesel(sql_type = Nullable<Text>)]
            unique_errors: Option<String>,
        }

        let mut conn = get_connection(&self.pool)?;
        let results: Vec<ProviderSyncStatsRow> = diesel::sql_query(
            r#"
            SELECT
                data_source as provider_id,
                COUNT(*) as asset_count,
                SUM(CASE WHEN last_error IS NOT NULL THEN 1 ELSE 0 END) as error_count,
                MAX(last_synced_at) as last_synced_at,
                (
                    SELECT qss2.last_error
                    FROM wf_quote_sync_state qss2
                    WHERE qss2.data_source = wf_quote_sync_state.data_source
                      AND qss2.last_error IS NOT NULL
                    ORDER BY qss2.updated_at DESC
                    LIMIT 1
                ) as last_error,
                (
                    SELECT STRING_AGG(DISTINCT qss3.last_error, '||')
                    FROM wf_quote_sync_state qss3
                    WHERE qss3.data_source = wf_quote_sync_state.data_source
                      AND qss3.last_error IS NOT NULL
                ) as unique_errors
            FROM wf_quote_sync_state
            GROUP BY data_source
            ORDER BY data_source
            "#,
        )
        .load(&mut conn)
        .map_err(StorageError::from)?;

        Ok(results
            .into_iter()
            .map(|row| ProviderSyncStats {
                provider_id: row.provider_id,
                asset_count: row.asset_count,
                error_count: row.error_count,
                last_synced_at: row.last_synced_at,
                last_error: row.last_error,
                unique_errors: row
                    .unique_errors
                    .map(|s| s.split("||").map(|e| e.trim().to_string()).collect())
                    .unwrap_or_default(),
            })
            .collect())
    }

    fn get_all(&self) -> Result<Vec<QuoteSyncState>> {
        let mut conn = get_connection(&self.pool)?;
        let results = qss_dsl::wf_quote_sync_state
            .order(qss_dsl::sync_priority.desc())
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(QuoteSyncState::from).collect())
    }

    fn get_by_asset_id(&self, asset_id: &str) -> Result<Option<QuoteSyncState>> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::asset_id.eq(parsed_asset_id))
            .select(QuoteSyncStateDB::as_select())
            .first::<QuoteSyncStateDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(QuoteSyncState::from))
    }

    fn get_by_asset_ids(&self, asset_ids: &[String]) -> Result<HashMap<String, QuoteSyncState>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let parsed_ids = asset_ids
            .iter()
            .map(|value| parse_quote_sync_asset_id(value))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;
        let results = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::asset_id.eq_any(parsed_ids))
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results
            .into_iter()
            .map(QuoteSyncState::from)
            .map(|state| (state.asset_id.clone(), state))
            .collect())
    }

    fn get_active_assets(&self) -> Result<Vec<QuoteSyncState>> {
        let mut conn = get_connection(&self.pool)?;
        let results = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::position_closed_date.is_null())
            .order(qss_dsl::sync_priority.desc())
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(QuoteSyncState::from).collect())
    }

    fn get_assets_needing_sync(&self, grace_period_days: i64) -> Result<Vec<QuoteSyncState>> {
        let mut conn = get_connection(&self.pool)?;
        let grace_cutoff = Utc::now().date_naive() - chrono::Duration::days(grace_period_days);
        let results = qss_dsl::wf_quote_sync_state
            .filter(
                qss_dsl::position_closed_date
                    .is_null()
                    .or(qss_dsl::position_closed_date.ge(grace_cutoff)),
            )
            .order(qss_dsl::sync_priority.desc())
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(QuoteSyncState::from).collect())
    }

    async fn upsert(&self, state: &QuoteSyncState) -> Result<QuoteSyncState> {
        let db_state = QuoteSyncStateDB::from_domain(state)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(qss_dsl::wf_quote_sync_state)
            .values(&db_state)
            .on_conflict(qss_dsl::asset_id)
            .do_update()
            .set(&db_state)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        let result = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::asset_id.eq(db_state.asset_id))
            .select(QuoteSyncStateDB::as_select())
            .first::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(result.into())
    }

    async fn upsert_batch(&self, states: &[QuoteSyncState]) -> Result<usize> {
        if states.is_empty() {
            return Ok(0);
        }

        let db_states = states
            .iter()
            .map(QuoteSyncStateDB::from_domain)
            .collect::<Result<Vec<_>>>()?;
        let mut total = 0;
        let mut conn = get_connection(&self.pool)?;

        for chunk in db_states.chunks(500) {
            total += diesel::insert_into(qss_dsl::wf_quote_sync_state)
                .values(chunk)
                .on_conflict(qss_dsl::asset_id)
                .do_update()
                .set((
                    qss_dsl::position_closed_date.eq(diesel::upsert::excluded(qss_dsl::position_closed_date)),
                    qss_dsl::last_synced_at.eq(diesel::upsert::excluded(qss_dsl::last_synced_at)),
                    qss_dsl::data_source.eq(diesel::upsert::excluded(qss_dsl::data_source)),
                    qss_dsl::sync_priority.eq(diesel::upsert::excluded(qss_dsl::sync_priority)),
                    qss_dsl::error_count.eq(diesel::upsert::excluded(qss_dsl::error_count)),
                    qss_dsl::last_error.eq(diesel::upsert::excluded(qss_dsl::last_error)),
                    qss_dsl::profile_enriched_at.eq(diesel::upsert::excluded(qss_dsl::profile_enriched_at)),
                    qss_dsl::updated_at.eq(diesel::upsert::excluded(qss_dsl::updated_at)),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(total)
    }

    async fn update_after_sync(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        let now = Utc::now();
        let mut conn = get_connection(&self.pool)?;
        let update = QuoteSyncStateUpdateDB {
            last_synced_at: Some(Some(now)),
            error_count: Some(0),
            last_error: Some(None),
            updated_at: Some(now),
            ..Default::default()
        };

        diesel::update(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .set(&update)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn update_after_failure(&self, asset_id: &str, error: &str) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        let error_owned = error.to_string();
        let mut conn = get_connection(&self.pool)?;
        let current = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::asset_id.eq(parsed_asset_id))
            .select(QuoteSyncStateDB::as_select())
            .first::<QuoteSyncStateDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        let update = QuoteSyncStateUpdateDB {
            error_count: Some(current.map(|state| state.error_count + 1).unwrap_or(1)),
            last_error: Some(Some(error_owned)),
            updated_at: Some(Utc::now()),
            ..Default::default()
        };

        diesel::update(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .set(&update)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn mark_inactive(&self, asset_id: &str, closed_date: NaiveDate) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        debug!("Marking asset {} as inactive (closed: {})", asset_id, closed_date);
        let update = QuoteSyncStateUpdateDB {
            position_closed_date: Some(Some(closed_date)),
            sync_priority: Some(50),
            updated_at: Some(Utc::now()),
            ..Default::default()
        };
        let mut conn = get_connection(&self.pool)?;

        diesel::update(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .set(&update)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn mark_active(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        debug!("Marking asset {} as active", asset_id);
        let update = QuoteSyncStateUpdateDB {
            position_closed_date: Some(None),
            sync_priority: Some(100),
            updated_at: Some(Utc::now()),
            ..Default::default()
        };
        let mut conn = get_connection(&self.pool)?;

        diesel::update(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .set(&update)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn delete(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn delete_all(&self) -> Result<usize> {
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(qss_dsl::wf_quote_sync_state)
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn mark_profile_enriched(&self, asset_id: &str) -> Result<()> {
        let parsed_asset_id = parse_quote_sync_asset_id(asset_id)?;
        debug!("Marking profile enriched for asset {}", asset_id);
        let now = Utc::now();
        let update = QuoteSyncStateUpdateDB {
            profile_enriched_at: Some(Some(now)),
            updated_at: Some(now),
            ..Default::default()
        };
        let mut conn = get_connection(&self.pool)?;

        diesel::update(qss_dsl::wf_quote_sync_state.filter(qss_dsl::asset_id.eq(parsed_asset_id)))
            .set(&update)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn get_assets_needing_profile_enrichment(&self) -> Result<Vec<QuoteSyncState>> {
        let mut conn = get_connection(&self.pool)?;
        let results = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::profile_enriched_at.is_null())
            .order(qss_dsl::sync_priority.desc())
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(QuoteSyncState::from).collect())
    }

    fn get_with_errors(&self) -> Result<Vec<QuoteSyncState>> {
        let mut conn = get_connection(&self.pool)?;
        let results = qss_dsl::wf_quote_sync_state
            .filter(qss_dsl::error_count.gt(0))
            .order(qss_dsl::error_count.desc())
            .select(QuoteSyncStateDB::as_select())
            .load::<QuoteSyncStateDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(QuoteSyncState::from).collect())
    }
}