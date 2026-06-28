use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::dsl::{max, min};
use diesel::prelude::*;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::market_data::model::{
    MarketDataProviderSettingDB, QuoteDB, UpdateMarketDataProviderSettingDB,
};
use crate::schema::wf_market_data_providers::dsl as providers_dsl;
use crate::schema::wf_quotes::dsl as quotes_dsl;
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::quotes::store::{ProviderSettingsStore, QuoteStore};
use wealthfolio_core::quotes::types::{AssetId, Day, QuoteSource};
use wealthfolio_core::quotes::{
    LatestQuotePair, MarketDataProviderSetting, Quote, UpdateMarketDataProviderSetting,
};
use wealthfolio_core::Result;

pub struct MarketDataRepository {
    pool: Arc<DbPool>,
}

impl MarketDataRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn parse_asset_uuid(asset_id: &AssetId) -> Result<uuid::Uuid> {
        uuid::Uuid::parse_str(asset_id.as_str())
            .map_err(|err| ValidationError::InvalidInput(format!("Invalid asset_id UUID: {err}")).into())
    }

    fn parse_asset_uuid_str(asset_id: &str) -> Result<uuid::Uuid> {
        uuid::Uuid::parse_str(asset_id)
            .map_err(|err| ValidationError::InvalidInput(format!("Invalid asset_id UUID: {err}")).into())
    }

    fn latest_rows_for_assets(
        &self,
        asset_ids: &[uuid::Uuid],
        source: Option<&QuoteSource>,
    ) -> Result<Vec<QuoteDB>> {
        if asset_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = get_connection(&self.pool)?;
        let mut query = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq_any(asset_ids))
            .order((quotes_dsl::asset_id.asc(), quotes_dsl::day.desc(), quotes_dsl::timestamp_.desc()))
            .select(QuoteDB::as_select())
            .into_boxed();

        if let Some(src) = source {
            query = query.filter(quotes_dsl::source.eq(src.to_storage_string()));
        }

        query.load::<QuoteDB>(&mut conn).map_err(StorageError::from).map_err(Into::into)
    }
}

#[async_trait]
impl QuoteStore for MarketDataRepository {
    async fn save_quote(&self, quote: &Quote) -> Result<Quote> {
        let db_row = QuoteDB::try_from(quote)?;
        let mut conn = get_connection(&self.pool)?;
        let update_open = db_row.open.clone();
        let update_high = db_row.high.clone();
        let update_low = db_row.low.clone();
        let update_close = db_row.close.clone();
        let update_adjclose = db_row.adjclose.clone();
        let update_volume = db_row.volume;
        let update_currency = db_row.currency.clone();
        let update_notes = db_row.notes.clone();
        let update_created_at = db_row.created_at;
        let update_timestamp = db_row.timestamp;

        let saved_row = diesel::insert_into(quotes_dsl::wf_quotes)
            .values(&db_row)
            .on_conflict((quotes_dsl::asset_id, quotes_dsl::day, quotes_dsl::source))
            .do_update()
            .set((
                quotes_dsl::open.eq(update_open),
                quotes_dsl::high.eq(update_high),
                quotes_dsl::low.eq(update_low),
                quotes_dsl::close.eq(update_close),
                quotes_dsl::adjclose.eq(update_adjclose),
                quotes_dsl::volume.eq(update_volume),
                quotes_dsl::currency.eq(update_currency),
                quotes_dsl::notes.eq(update_notes),
                quotes_dsl::created_at.eq(update_created_at),
                quotes_dsl::timestamp_.eq(update_timestamp),
            ))
            .returning(QuoteDB::as_returning())
            .get_result::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(Quote::from(saved_row))
    }

    async fn delete_quote(&self, quote_id: &str) -> Result<()> {
        let parsed_id = match uuid::Uuid::parse_str(quote_id) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(quotes_dsl::wf_quotes.filter(quotes_dsl::id.eq(parsed_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn upsert_quotes(&self, input_quotes: &[Quote]) -> Result<usize> {
        use diesel::upsert::excluded;

        if input_quotes.is_empty() {
            return Ok(0);
        }

        let db_rows: Vec<QuoteDB> = input_quotes
            .iter()
            .map(QuoteDB::try_from)
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        let provider_pairs: HashSet<(uuid::Uuid, NaiveDate)> = db_rows
            .iter()
            .filter(|row| !row.source.eq_ignore_ascii_case("MANUAL"))
            .map(|row| (row.asset_id, row.day))
            .collect();

        let db_rows = if provider_pairs.is_empty() {
            db_rows
        } else {
            let asset_ids: Vec<uuid::Uuid> = provider_pairs.iter().map(|(asset_id, _)| *asset_id).collect();
            let days: Vec<NaiveDate> = provider_pairs.iter().map(|(_, day)| *day).collect();
            let manual_days: HashSet<(uuid::Uuid, NaiveDate)> = quotes_dsl::wf_quotes
                .filter(quotes_dsl::source.eq("MANUAL"))
                .filter(quotes_dsl::asset_id.eq_any(&asset_ids))
                .filter(quotes_dsl::day.eq_any(&days))
                .select((quotes_dsl::asset_id, quotes_dsl::day))
                .load::<(uuid::Uuid, NaiveDate)>(&mut conn)
                .map_err(StorageError::from)?
                .into_iter()
                .collect();

            db_rows
                .into_iter()
                .filter(|row| {
                    row.source.eq_ignore_ascii_case("MANUAL")
                        || !manual_days.contains(&(row.asset_id, row.day))
                })
                .collect()
        };

        let mut total_upserted = 0usize;
        for row in db_rows {
            total_upserted += diesel::insert_into(quotes_dsl::wf_quotes)
                .values(&row)
                .on_conflict((quotes_dsl::asset_id, quotes_dsl::day, quotes_dsl::source))
                .do_update()
                .set((
                    quotes_dsl::open.eq(excluded(quotes_dsl::open)),
                    quotes_dsl::high.eq(excluded(quotes_dsl::high)),
                    quotes_dsl::low.eq(excluded(quotes_dsl::low)),
                    quotes_dsl::close.eq(excluded(quotes_dsl::close)),
                    quotes_dsl::adjclose.eq(excluded(quotes_dsl::adjclose)),
                    quotes_dsl::volume.eq(excluded(quotes_dsl::volume)),
                    quotes_dsl::currency.eq(excluded(quotes_dsl::currency)),
                    quotes_dsl::notes.eq(excluded(quotes_dsl::notes)),
                    quotes_dsl::created_at.eq(excluded(quotes_dsl::created_at)),
                    quotes_dsl::timestamp_.eq(excluded(quotes_dsl::timestamp_)),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(total_upserted)
    }

    async fn delete_quotes_for_asset(&self, asset_id: &AssetId) -> Result<usize> {
        let parsed_asset_id = Self::parse_asset_uuid(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(quotes_dsl::wf_quotes.filter(quotes_dsl::asset_id.eq(parsed_asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn delete_provider_quotes_for_asset(&self, asset_id: &AssetId) -> Result<usize> {
        let parsed_asset_id = Self::parse_asset_uuid(asset_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            quotes_dsl::wf_quotes
                .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
                .filter(quotes_dsl::source.ne("MANUAL")),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)
        .map_err(Into::into)
    }

    fn latest(&self, asset_id: &AssetId, source: Option<&QuoteSource>) -> Result<Option<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid(asset_id)?;
        let mut query = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .order((quotes_dsl::day.desc(), quotes_dsl::timestamp_.desc()))
            .select(QuoteDB::as_select())
            .into_boxed();
        if let Some(src) = source {
            query = query.filter(quotes_dsl::source.eq(src.to_storage_string()));
        }
        let result = query.first::<QuoteDB>(&mut conn).optional().map_err(StorageError::from)?;
        Ok(result.map(Quote::from))
    }

    fn range(&self, asset_id: &AssetId, start: Day, end: Day, source: Option<&QuoteSource>) -> Result<Vec<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid(asset_id)?;
        let mut query = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .filter(quotes_dsl::day.ge(start.date()))
            .filter(quotes_dsl::day.le(end.date()))
            .order(quotes_dsl::day.asc())
            .select(QuoteDB::as_select())
            .into_boxed();
        if let Some(src) = source {
            query = query.filter(quotes_dsl::source.eq(src.to_storage_string()));
        }
        let results = query.load::<QuoteDB>(&mut conn).map_err(StorageError::from)?;
        Ok(results.into_iter().map(Quote::from).collect())
    }

    fn latest_batch(&self, asset_ids: &[AssetId], source: Option<&QuoteSource>) -> Result<HashMap<AssetId, Quote>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let parsed_asset_ids = asset_ids
            .iter()
            .map(Self::parse_asset_uuid)
            .collect::<Result<Vec<_>>>()?;
        let rows = self.latest_rows_for_assets(&parsed_asset_ids, source)?;
        let mut result = HashMap::new();
        for row in rows {
            result.entry(AssetId::new(row.asset_id.to_string())).or_insert_with(|| Quote::from(row));
        }
        Ok(result)
    }

    fn latest_with_previous(&self, asset_ids: &[AssetId]) -> Result<HashMap<AssetId, LatestQuotePair>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let parsed_asset_ids = asset_ids
            .iter()
            .map(Self::parse_asset_uuid)
            .collect::<Result<Vec<_>>>()?;
        let rows = self.latest_rows_for_assets(&parsed_asset_ids, None)?;
        let mut grouped: HashMap<AssetId, Vec<Quote>> = HashMap::new();
        for row in rows {
            let asset_id = AssetId::new(row.asset_id.to_string());
            let quotes = grouped.entry(asset_id).or_default();
            if quotes.len() < 2 {
                quotes.push(Quote::from(row));
            }
        }
        Ok(grouped
            .into_iter()
            .filter_map(|(asset_id, mut quotes)| {
                if quotes.is_empty() {
                    None
                } else {
                    let latest = quotes.remove(0);
                    let previous = if quotes.is_empty() { None } else { Some(quotes.remove(0)) };
                    Some((asset_id, LatestQuotePair { latest, previous }))
                }
            })
            .collect())
    }

    fn get_quote_bounds_for_assets(&self, asset_ids: &[String], source: &str) -> Result<HashMap<String, (NaiveDate, NaiveDate)>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let parsed_ids = asset_ids
            .iter()
            .map(|id| Self::parse_asset_uuid_str(id))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;
        let rows = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq_any(parsed_ids))
            .filter(quotes_dsl::source.eq(source))
            .group_by(quotes_dsl::asset_id)
            .select((quotes_dsl::asset_id, min(quotes_dsl::day), max(quotes_dsl::day)))
            .load::<(uuid::Uuid, Option<NaiveDate>, Option<NaiveDate>)>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(rows
            .into_iter()
            .filter_map(|(asset_id, min_day, max_day)| Some((asset_id.to_string(), (min_day?, max_day?))))
            .collect())
    }

    fn get_latest_quote(&self, symbol: &str) -> Result<Quote> {
        let asset_id = AssetId::new(symbol.to_string());
        self.latest(&asset_id, None)?.ok_or_else(|| wealthfolio_core::errors::Error::Database(
            wealthfolio_core::errors::DatabaseError::NotFound(format!("No quote found in database for symbol: {}", symbol)),
        ))
    }

    fn get_latest_quotes(&self, symbols: &[String]) -> Result<HashMap<String, Quote>> {
        let asset_ids: Vec<AssetId> = symbols.iter().cloned().map(AssetId::new).collect();
        Ok(self.latest_batch(&asset_ids, None)?
            .into_iter()
            .map(|(asset_id, quote)| (asset_id.to_string(), quote))
            .collect())
    }

    fn get_latest_quotes_pair(&self, symbols: &[String]) -> Result<HashMap<String, LatestQuotePair>> {
        let asset_ids: Vec<AssetId> = symbols.iter().cloned().map(AssetId::new).collect();
        Ok(self.latest_with_previous(&asset_ids)?
            .into_iter()
            .map(|(asset_id, pair)| (asset_id.to_string(), pair))
            .collect())
    }

    fn get_latest_quote_before(&self, symbol: &str, before: NaiveDate) -> Result<Option<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid_str(symbol)?;
        let result = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .filter(quotes_dsl::day.lt(before))
            .order((quotes_dsl::day.desc(), quotes_dsl::timestamp_.desc()))
            .select(QuoteDB::as_select())
            .first::<QuoteDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(Quote::from))
    }

    fn get_historical_quotes(&self, symbol: &str) -> Result<Vec<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid_str(symbol)?;
        let results = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .order(quotes_dsl::day.desc())
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Quote::from).collect())
    }

    fn get_all_historical_quotes(&self) -> Result<Vec<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let results = quotes_dsl::wf_quotes
            .order(quotes_dsl::day.desc())
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Quote::from).collect())
    }

    fn get_quotes_in_range(&self, symbol: &str, start: NaiveDate, end: NaiveDate) -> Result<Vec<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid_str(symbol)?;
        let results = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .filter(quotes_dsl::day.ge(start))
            .filter(quotes_dsl::day.le(end))
            .order(quotes_dsl::day.asc())
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Quote::from).collect())
    }

    fn find_duplicate_quotes(&self, symbol: &str, date: NaiveDate) -> Result<Vec<Quote>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_asset_id = Self::parse_asset_uuid_str(symbol)?;
        let results = quotes_dsl::wf_quotes
            .filter(quotes_dsl::asset_id.eq(parsed_asset_id))
            .filter(quotes_dsl::day.eq(date))
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Quote::from).collect())
    }
}

impl ProviderSettingsStore for MarketDataRepository {
    fn get_all_providers(&self) -> Result<Vec<MarketDataProviderSetting>> {
        let mut conn = get_connection(&self.pool)?;
        let db_results = providers_dsl::wf_market_data_providers
            .order(providers_dsl::priority.desc())
            .select(MarketDataProviderSettingDB::as_select())
            .load::<MarketDataProviderSettingDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(db_results.into_iter().map(MarketDataProviderSetting::from).collect())
    }

    fn get_provider(&self, id: &str) -> Result<MarketDataProviderSetting> {
        let mut conn = get_connection(&self.pool)?;
        let db_result = providers_dsl::wf_market_data_providers
            .find(id)
            .select(MarketDataProviderSettingDB::as_select())
            .first::<MarketDataProviderSettingDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(MarketDataProviderSetting::from(db_result))
    }

    fn update_provider(&self, id: &str, changes: UpdateMarketDataProviderSetting) -> Result<MarketDataProviderSetting> {
        let mut conn = get_connection(&self.pool)?;
        let changes_db = UpdateMarketDataProviderSettingDB { priority: changes.priority, enabled: changes.enabled };
        diesel::update(providers_dsl::wf_market_data_providers.find(id))
            .set(&changes_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        let db_result = providers_dsl::wf_market_data_providers
            .find(id)
            .select(MarketDataProviderSettingDB::as_select())
            .first::<MarketDataProviderSettingDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(MarketDataProviderSetting::from(db_result))
    }
}