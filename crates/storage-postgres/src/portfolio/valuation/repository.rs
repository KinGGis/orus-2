use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::dsl::max;
use diesel::prelude::*;
use diesel::upsert::excluded;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::portfolio::valuation::model::DailyAccountValuationDB;
use crate::schema::wf_daily_account_valuation::dsl as valuations_dsl;
use crate::system_accounts::parse_account_id;
use wealthfolio_core::errors::Result;
use wealthfolio_core::portfolio::valuation::{DailyAccountValuation, ValuationRepositoryTrait};

pub struct ValuationRepository {
    pool: Arc<DbPool>,
}

impl ValuationRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn latest_rows_for_accounts(&self, account_ids: &[String]) -> Result<Vec<DailyAccountValuationDB>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let parsed_ids = account_ids
            .iter()
            .map(|account_id| parse_account_id(account_id))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        valuations_dsl::wf_daily_account_valuation
            .filter(valuations_dsl::account_id.eq_any(parsed_ids))
            .order((valuations_dsl::account_id.asc(), valuations_dsl::valuation_date.desc()))
            .select(DailyAccountValuationDB::as_select())
            .load::<DailyAccountValuationDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }
}

#[async_trait]
impl ValuationRepositoryTrait for ValuationRepository {
    async fn save_valuations(&self, valuation_records: &[DailyAccountValuation]) -> Result<()> {
        if valuation_records.is_empty() {
            return Ok(());
        }

        let db_rows = valuation_records
            .iter()
            .map(DailyAccountValuationDB::try_from)
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        for row in db_rows {
            diesel::insert_into(valuations_dsl::wf_daily_account_valuation)
                .values(&row)
                .on_conflict(valuations_dsl::id)
                .do_update()
                .set((
                    valuations_dsl::account_id.eq(excluded(valuations_dsl::account_id)),
                    valuations_dsl::valuation_date.eq(excluded(valuations_dsl::valuation_date)),
                    valuations_dsl::account_currency.eq(excluded(valuations_dsl::account_currency)),
                    valuations_dsl::base_currency.eq(excluded(valuations_dsl::base_currency)),
                    valuations_dsl::fx_rate_to_base.eq(excluded(valuations_dsl::fx_rate_to_base)),
                    valuations_dsl::cash_balance.eq(excluded(valuations_dsl::cash_balance)),
                    valuations_dsl::investment_market_value.eq(excluded(valuations_dsl::investment_market_value)),
                    valuations_dsl::total_value.eq(excluded(valuations_dsl::total_value)),
                    valuations_dsl::cost_basis.eq(excluded(valuations_dsl::cost_basis)),
                    valuations_dsl::net_contribution.eq(excluded(valuations_dsl::net_contribution)),
                    valuations_dsl::calculated_at.eq(excluded(valuations_dsl::calculated_at)),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(())
    }

    fn get_historical_valuations(
        &self,
        account_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DailyAccountValuation>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let mut query = valuations_dsl::wf_daily_account_valuation
            .filter(valuations_dsl::account_id.eq(parsed_account_id))
            .order(valuations_dsl::valuation_date.asc())
            .select(DailyAccountValuationDB::as_select())
            .into_boxed();

        if let Some(start) = start_date {
            query = query.filter(valuations_dsl::valuation_date.ge(start));
        }
        if let Some(end) = end_date {
            query = query.filter(valuations_dsl::valuation_date.le(end));
        }

        query
            .load::<DailyAccountValuationDB>(&mut conn)
            .map(|rows| rows.into_iter().map(DailyAccountValuation::from).collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn load_latest_valuation_date(&self, account_id: &str) -> Result<Option<NaiveDate>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;

        valuations_dsl::wf_daily_account_valuation
            .filter(valuations_dsl::account_id.eq(parsed_account_id))
            .select(max(valuations_dsl::valuation_date))
            .first::<Option<NaiveDate>>(&mut conn)
            .optional()
            .map(|value| value.flatten())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn delete_valuations_for_account(
        &self,
        account_id: &str,
        since_date: Option<NaiveDate>,
    ) -> Result<()> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;

        let query = valuations_dsl::wf_daily_account_valuation
            .filter(valuations_dsl::account_id.eq(parsed_account_id));

        match since_date {
            None => {
                diesel::delete(query)
                    .execute(&mut conn)
                    .map_err(StorageError::from)?;
            }
            Some(date) => {
                diesel::delete(query.filter(valuations_dsl::valuation_date.ge(date)))
                    .execute(&mut conn)
                    .map_err(StorageError::from)?;
            }
        }

        Ok(())
    }

    fn get_latest_valuations(&self, account_ids: &[String]) -> Result<Vec<DailyAccountValuation>> {
        let rows = self.latest_rows_for_accounts(account_ids)?;
        let mut results_map: HashMap<String, DailyAccountValuation> = HashMap::new();

        for row in rows {
            let valuation = DailyAccountValuation::from(row);
            results_map.entry(valuation.account_id.clone()).or_insert(valuation);
        }

        Ok(account_ids
            .iter()
            .filter_map(|account_id| results_map.remove(account_id))
            .collect())
    }

    fn get_valuations_on_date(
        &self,
        account_ids: &[String],
        date: NaiveDate,
    ) -> Result<Vec<DailyAccountValuation>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let parsed_ids = account_ids
            .iter()
            .map(|account_id| parse_account_id(account_id))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        valuations_dsl::wf_daily_account_valuation
            .filter(valuations_dsl::account_id.eq_any(parsed_ids))
            .filter(valuations_dsl::valuation_date.eq(date))
            .select(DailyAccountValuationDB::as_select())
            .load::<DailyAccountValuationDB>(&mut conn)
            .map(|rows| rows.into_iter().map(DailyAccountValuation::from).collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }
}