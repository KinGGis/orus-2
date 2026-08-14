use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::fx::model::{AssetDB, InsertableAssetDB, QuoteDB};
use crate::schema::{wf_assets, wf_quotes};
use wealthfolio_core::assets::{AssetKind, NewAsset};
use wealthfolio_core::errors::{DatabaseError, ValidationError};
use wealthfolio_core::fx::{ExchangeRate, FxRepositoryTrait};
use wealthfolio_core::quotes::{DataSource, Quote};
use wealthfolio_core::{Error, Result};

#[derive(Clone)]
pub struct FxRepository {
    pool: Arc<DbPool>,
}

impl FxRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
        Uuid::parse_str(value).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid {field} UUID: {err}"
            )))
        })
    }

    fn load_fx_assets(&self) -> Result<Vec<AssetDB>> {
        let mut conn = get_connection(&self.pool)?;
        wf_assets::table
            .filter(wf_assets::kind.eq(AssetKind::Fx.as_db_str()))
            .order_by(wf_assets::display_code.asc())
            .load::<AssetDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn latest_quotes_for_assets(&self, asset_ids: &[Uuid]) -> Result<HashMap<Uuid, QuoteDB>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = get_connection(&self.pool)?;
        let quotes = wf_quotes::table
            .filter(wf_quotes::asset_id.eq_any(asset_ids))
            .order_by((wf_quotes::asset_id.asc(), wf_quotes::timestamp_.desc()))
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;

        let mut latest = HashMap::new();
        for quote in quotes {
            latest.entry(quote.asset_id).or_insert(quote);
        }
        Ok(latest)
    }

    fn asset_to_exchange_rate(asset: &AssetDB, quote: Option<&QuoteDB>) -> ExchangeRate {
        match quote {
            Some(quote_db) => ExchangeRate {
                id: asset.id.to_string(),
                from_currency: asset.instrument_symbol.clone().unwrap_or_default(),
                to_currency: asset.quote_ccy.clone(),
                rate: Quote::from(quote_db.clone()).close,
                source: DataSource::from(quote_db.source.as_str()),
                timestamp: quote_db.timestamp_,
            },
            None => ExchangeRate {
                id: asset.id.to_string(),
                from_currency: asset.instrument_symbol.clone().unwrap_or_default(),
                to_currency: asset.quote_ccy.clone(),
                rate: Decimal::ZERO,
                source: asset
                    .provider_config
                    .as_ref()
                    .and_then(|value| value.get("preferred_provider"))
                    .and_then(|value| value.as_str())
                    .map(DataSource::from)
                    .unwrap_or(DataSource::Manual),
                timestamp: asset.updated_at,
            },
        }
    }

    fn get_exchange_rate_internal(&self, key_or_id: &str) -> Result<Option<ExchangeRate>> {
        let mut conn = get_connection(&self.pool)?;

        let maybe_uuid = Uuid::parse_str(key_or_id).ok();
        let asset = wf_assets::table
            .filter(
                wf_assets::instrument_key
                    .eq(key_or_id)
                    .or(maybe_uuid.map(|id| wf_assets::id.eq(id)).unwrap_or(wf_assets::id.eq(Uuid::nil()))),
            )
            .filter(wf_assets::kind.eq(AssetKind::Fx.as_db_str()))
            .first::<AssetDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        let Some(asset) = asset else {
            return Ok(None);
        };

        let quote = wf_quotes::table
            .filter(wf_quotes::asset_id.eq(asset.id))
            .order_by(wf_quotes::timestamp_.desc())
            .select(QuoteDB::as_select())
            .first::<QuoteDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(Some(Self::asset_to_exchange_rate(&asset, quote.as_ref())))
    }

    fn lookup_asset_id(&self, symbol_or_asset_id: &str) -> Result<Uuid> {
        if let Ok(id) = Uuid::parse_str(symbol_or_asset_id) {
            return Ok(id);
        }

        let mut conn = get_connection(&self.pool)?;
        wf_assets::table
            .filter(wf_assets::instrument_key.eq(symbol_or_asset_id))
            .select(wf_assets::id)
            .first::<Uuid>(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }
}

#[async_trait]
impl FxRepositoryTrait for FxRepository {
    fn get_latest_exchange_rates(&self) -> Result<Vec<ExchangeRate>> {
        let assets = self.load_fx_assets()?;
        let asset_ids: Vec<Uuid> = assets.iter().map(|asset| asset.id).collect();
        let latest_quotes = self.latest_quotes_for_assets(&asset_ids)?;

        Ok(assets
            .iter()
            .map(|asset| Self::asset_to_exchange_rate(asset, latest_quotes.get(&asset.id)))
            .collect())
    }

    fn get_historical_exchange_rates(&self) -> Result<Vec<ExchangeRate>> {
        let mut conn = get_connection(&self.pool)?;
        let rows = wf_quotes::table
            .inner_join(wf_assets::table.on(wf_quotes::asset_id.eq(wf_assets::id)))
            .filter(wf_assets::kind.eq(AssetKind::Fx.as_db_str()))
            .select((QuoteDB::as_select(), AssetDB::as_select()))
            .order_by((wf_quotes::asset_id.asc(), wf_quotes::timestamp_.asc()))
            .load::<(QuoteDB, AssetDB)>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(rows
            .into_iter()
            .map(|(quote, asset)| ExchangeRate {
                id: asset.id.to_string(),
                from_currency: asset.instrument_symbol.unwrap_or_default(),
                to_currency: asset.quote_ccy,
                rate: Quote::from(quote.clone()).close,
                source: DataSource::from(quote.source.as_str()),
                timestamp: quote.timestamp_,
            })
            .collect())
    }

    fn get_latest_exchange_rate(&self, from: &str, to: &str) -> Result<Option<ExchangeRate>> {
        let key = ExchangeRate::make_instrument_key(from, to);
        self.get_exchange_rate_internal(&key)
    }

    fn get_latest_exchange_rate_by_symbol(&self, symbol: &str) -> Result<Option<ExchangeRate>> {
        self.get_exchange_rate_internal(symbol)
    }

    fn get_historical_quotes(
        &self,
        symbol: &str,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<Quote>> {
        let asset_id = self.lookup_asset_id(symbol)?;
        let mut conn = get_connection(&self.pool)?;
        let start_dt = Utc.from_utc_datetime(&start_date);
        let end_dt = Utc.from_utc_datetime(&end_date);

        let quotes = wf_quotes::table
            .filter(wf_quotes::asset_id.eq(asset_id))
            .filter(wf_quotes::timestamp_.ge(start_dt))
            .filter(wf_quotes::timestamp_.le(end_dt))
            .order_by(wf_quotes::timestamp_.asc())
            .select(QuoteDB::as_select())
            .load::<QuoteDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(quotes.into_iter().map(Quote::from).collect())
    }

    async fn add_quote(
        &self,
        symbol: String,
        date: String,
        rate: Decimal,
        source: String,
    ) -> Result<Quote> {
        let asset_id = self.lookup_asset_id(&symbol)?;
        let naive_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid date format: {err}"
            )))
        })?;
        let naive_dt = naive_date.and_hms_opt(16, 0, 0).ok_or_else(|| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Failed to create timestamp for {date}"
            )))
        })?;
        let timestamp = Utc.from_utc_datetime(&naive_dt);

        let mut conn = get_connection(&self.pool)?;
        let asset = wf_assets::table
            .filter(wf_assets::id.eq(asset_id))
            .first::<AssetDB>(&mut conn)
            .map_err(StorageError::from)?;

        let quote = Quote {
            id: Uuid::new_v4().to_string(),
            asset_id: asset_id.to_string(),
            timestamp,
            open: rate,
            high: rate,
            low: rate,
            close: rate,
            adjclose: rate,
            volume: Decimal::ZERO,
            currency: asset.instrument_symbol.unwrap_or_default(),
            data_source: DataSource::from(source.as_str()),
            created_at: Utc::now(),
            notes: None,
        };
        let quote_db = QuoteDB::from_quote(&quote).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid quote UUID payload: {err}"
            )))
        })?;
        let update_open = quote_db.open.clone();
        let update_high = quote_db.high.clone();
        let update_low = quote_db.low.clone();
        let update_close = quote_db.close.clone();
        let update_adjclose = quote_db.adjclose.clone();
        let update_volume = quote_db.volume;
        let update_currency = quote_db.currency.clone();
        let update_notes = quote_db.notes.clone();
        let update_created_at = quote_db.created_at;
        let update_timestamp = quote_db.timestamp_;

        diesel::insert_into(wf_quotes::table)
            .values(&quote_db)
            .on_conflict((wf_quotes::asset_id, wf_quotes::day, wf_quotes::source))
            .do_update()
            .set((
                wf_quotes::open.eq(update_open),
                wf_quotes::high.eq(update_high),
                wf_quotes::low.eq(update_low),
                wf_quotes::close.eq(update_close),
                wf_quotes::adjclose.eq(update_adjclose),
                wf_quotes::volume.eq(update_volume),
                wf_quotes::currency.eq(update_currency),
                wf_quotes::notes.eq(update_notes),
                wf_quotes::created_at.eq(update_created_at),
                wf_quotes::timestamp_.eq(update_timestamp),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(quote)
    }

    async fn save_exchange_rate(&self, rate: ExchangeRate) -> Result<ExchangeRate> {
        let quote = rate.to_quote();
        let quote_db = QuoteDB::from_quote(&quote).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid exchange rate payload: {err}"
            )))
        })?;
        let mut conn = get_connection(&self.pool)?;
        let update_open = quote_db.open.clone();
        let update_high = quote_db.high.clone();
        let update_low = quote_db.low.clone();
        let update_close = quote_db.close.clone();
        let update_adjclose = quote_db.adjclose.clone();
        let update_volume = quote_db.volume;
        let update_currency = quote_db.currency.clone();
        let update_notes = quote_db.notes.clone();
        let update_created_at = quote_db.created_at;
        let update_timestamp = quote_db.timestamp_;

        diesel::insert_into(wf_quotes::table)
            .values(&quote_db)
            .on_conflict((wf_quotes::asset_id, wf_quotes::day, wf_quotes::source))
            .do_update()
            .set((
                wf_quotes::open.eq(update_open),
                wf_quotes::high.eq(update_high),
                wf_quotes::low.eq(update_low),
                wf_quotes::close.eq(update_close),
                wf_quotes::adjclose.eq(update_adjclose),
                wf_quotes::volume.eq(update_volume),
                wf_quotes::currency.eq(update_currency),
                wf_quotes::notes.eq(update_notes),
                wf_quotes::created_at.eq(update_created_at),
                wf_quotes::timestamp_.eq(update_timestamp),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(rate)
    }

    async fn update_exchange_rate(&self, rate: &ExchangeRate) -> Result<ExchangeRate> {
        let quote = rate.to_quote();
        let quote_db = QuoteDB::from_quote(&quote).map_err(|err| {
            Error::Validation(ValidationError::InvalidInput(format!(
                "Invalid exchange rate payload: {err}"
            )))
        })?;
        let mut conn = get_connection(&self.pool)?;
        let update_open = quote_db.open.clone();
        let update_high = quote_db.high.clone();
        let update_low = quote_db.low.clone();
        let update_close = quote_db.close.clone();
        let update_adjclose = quote_db.adjclose.clone();
        let update_volume = quote_db.volume;
        let update_currency = quote_db.currency.clone();
        let update_notes = quote_db.notes.clone();
        let update_timestamp = quote_db.timestamp_;

        let updated = diesel::update(
            wf_quotes::table
                .filter(wf_quotes::asset_id.eq(quote_db.asset_id))
                .filter(wf_quotes::day.eq(quote_db.day))
                .filter(wf_quotes::source.eq(&quote_db.source)),
        )
        .set((
            wf_quotes::open.eq(update_open),
            wf_quotes::high.eq(update_high),
            wf_quotes::low.eq(update_low),
            wf_quotes::close.eq(update_close),
            wf_quotes::adjclose.eq(update_adjclose),
            wf_quotes::volume.eq(update_volume),
            wf_quotes::currency.eq(update_currency),
            wf_quotes::notes.eq(update_notes),
            wf_quotes::timestamp_.eq(update_timestamp),
        ))
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        if updated == 0 {
            return Err(Error::Database(DatabaseError::NotFound(format!(
                "Exchange rate quote not found for asset {}",
                quote_db.asset_id
            ))));
        }

        Ok(rate.clone())
    }

    async fn delete_exchange_rate(&self, rate_id: &str) -> Result<()> {
        let asset_id = Self::parse_uuid(rate_id, "rate_id")?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(wf_quotes::table.filter(wf_quotes::asset_id.eq(asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        diesel::delete(wf_assets::table.filter(wf_assets::id.eq(asset_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(())
    }

    async fn create_fx_asset(
        &self,
        from_currency: &str,
        to_currency: &str,
        source: &str,
    ) -> Result<String> {
        let expected_key = ExchangeRate::make_instrument_key(from_currency, to_currency);
        let mut conn = get_connection(&self.pool)?;

        let existing = wf_assets::table
            .filter(wf_assets::instrument_key.eq(&expected_key))
            .filter(wf_assets::kind.eq(AssetKind::Fx.as_db_str()))
            .first::<AssetDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        if let Some(asset) = existing {
            return Ok(asset.id.to_string());
        }

        let domain_asset = NewAsset::new_fx_asset(from_currency, to_currency, source);
        let now = Utc::now();
        let asset_id = Uuid::new_v4();
        let asset = InsertableAssetDB {
            id: asset_id,
            kind: domain_asset.kind.as_db_str().to_string(),
            name: domain_asset.name,
            display_code: domain_asset.display_code,
            notes: domain_asset.notes,
            metadata: domain_asset.metadata,
            is_active: domain_asset.is_active,
            quote_mode: domain_asset.quote_mode.as_db_str().to_string(),
            quote_ccy: domain_asset.quote_ccy,
            instrument_type: domain_asset
                .instrument_type
                .as_ref()
                .map(|value| value.as_db_str().to_string()),
            instrument_symbol: domain_asset.instrument_symbol,
            instrument_exchange_mic: domain_asset.instrument_exchange_mic,
            provider_config: domain_asset.provider_config,
            created_at: now,
            updated_at: now,
        };

        diesel::insert_into(wf_assets::table)
            .values(&asset)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(asset_id.to_string())
    }
}