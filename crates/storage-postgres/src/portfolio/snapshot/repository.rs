use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::upsert::excluded;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::portfolio::snapshot::model::AccountStateSnapshotDB;
use crate::schema::wf_accounts::dsl as accounts_dsl;
use crate::schema::wf_holdings_snapshots::dsl as snapshots_dsl;
use crate::system_accounts::{account_id_to_domain, parse_account_id, total_portfolio_account_uuid};
use wealthfolio_core::constants::PORTFOLIO_TOTAL_ACCOUNT_ID;
use wealthfolio_core::portfolio::snapshot::{AccountStateSnapshot, SnapshotRepositoryTrait};
use wealthfolio_core::Result;

const SOURCE_CALCULATED: &str = "CALCULATED";

pub struct SnapshotRepository {
    pool: Arc<DbPool>,
}

impl SnapshotRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn latest_snapshot_rows(
        &self,
        account_ids: &[String],
        target_date: Option<NaiveDate>,
    ) -> Result<Vec<AccountStateSnapshotDB>> {
        if account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let parsed_ids = account_ids
            .iter()
            .map(|account_id| parse_account_id(account_id))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        let mut query = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq_any(parsed_ids))
            .order((snapshots_dsl::account_id.asc(), snapshots_dsl::snapshot_date.desc()))
            .select(AccountStateSnapshotDB::as_select())
            .into_boxed();

        if let Some(date) = target_date {
            query = query.filter(snapshots_dsl::snapshot_date.le(date));
        }

        query
            .load::<AccountStateSnapshotDB>(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn get_anchor_snapshot_dates_for_account_in_range(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<HashSet<NaiveDate>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        snapshots_dsl::wf_holdings_snapshots
            .select(snapshots_dsl::snapshot_date)
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .filter(snapshots_dsl::snapshot_date.ge(start_date))
            .filter(snapshots_dsl::snapshot_date.le(end_date))
            .filter(snapshots_dsl::source.ne(SOURCE_CALCULATED))
            .load::<NaiveDate>(&mut conn)
            .map(|dates| dates.into_iter().collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn get_anchor_snapshot_dates_for_account(
        &self,
        account_id: &str,
    ) -> Result<HashSet<NaiveDate>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        snapshots_dsl::wf_holdings_snapshots
            .select(snapshots_dsl::snapshot_date)
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .filter(snapshots_dsl::source.ne(SOURCE_CALCULATED))
            .load::<NaiveDate>(&mut conn)
            .map(|dates| dates.into_iter().collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn upsert_snapshot_rows(&self, snapshots: &[AccountStateSnapshot]) -> Result<()> {
        if snapshots.is_empty() {
            return Ok(());
        }

        let db_rows = snapshots
            .iter()
            .map(AccountStateSnapshotDB::try_from)
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;

        for row in db_rows {
            diesel::insert_into(snapshots_dsl::wf_holdings_snapshots)
                .values(&row)
                .on_conflict(snapshots_dsl::id)
                .do_update()
                .set((
                    snapshots_dsl::account_id.eq(excluded(snapshots_dsl::account_id)),
                    snapshots_dsl::snapshot_date.eq(excluded(snapshots_dsl::snapshot_date)),
                    snapshots_dsl::currency.eq(excluded(snapshots_dsl::currency)),
                    snapshots_dsl::positions.eq(excluded(snapshots_dsl::positions)),
                    snapshots_dsl::cash_balances.eq(excluded(snapshots_dsl::cash_balances)),
                    snapshots_dsl::cost_basis.eq(excluded(snapshots_dsl::cost_basis)),
                    snapshots_dsl::net_contribution.eq(excluded(snapshots_dsl::net_contribution)),
                    snapshots_dsl::calculated_at.eq(excluded(snapshots_dsl::calculated_at)),
                    snapshots_dsl::net_contribution_base.eq(excluded(snapshots_dsl::net_contribution_base)),
                    snapshots_dsl::cash_total_account_currency.eq(excluded(snapshots_dsl::cash_total_account_currency)),
                    snapshots_dsl::cash_total_base_currency.eq(excluded(snapshots_dsl::cash_total_base_currency)),
                    snapshots_dsl::source.eq(excluded(snapshots_dsl::source)),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
        }

        Ok(())
    }

    pub async fn save_or_update_snapshot_impl(&self, snapshot: &AccountStateSnapshot) -> Result<()> {
        self.upsert_snapshot_rows(std::slice::from_ref(snapshot)).await
    }

    pub fn get_non_calculated_snapshot_count_impl(&self, account_id: &str) -> Result<usize> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let count = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .filter(snapshots_dsl::source.ne(SOURCE_CALCULATED))
            .select(count_star())
            .first::<i64>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(count as usize)
    }

    pub fn get_earliest_non_calculated_snapshot_impl(
        &self,
        account_id: &str,
    ) -> Result<Option<AccountStateSnapshot>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .filter(snapshots_dsl::source.ne(SOURCE_CALCULATED))
            .order(snapshots_dsl::snapshot_date.asc())
            .select(AccountStateSnapshotDB::as_select())
            .first::<AccountStateSnapshotDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(AccountStateSnapshot::from))
    }
}

#[async_trait]
impl SnapshotRepositoryTrait for SnapshotRepository {
    async fn save_snapshots(&self, snapshots: &[AccountStateSnapshot]) -> Result<()> {
        self.upsert_snapshot_rows(snapshots).await
    }

    fn get_snapshots_by_account(
        &self,
        account_id: &str,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<AccountStateSnapshot>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let mut query = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .order(snapshots_dsl::snapshot_date.asc())
            .select(AccountStateSnapshotDB::as_select())
            .into_boxed();

        if let Some(start) = start_date {
            query = query.filter(snapshots_dsl::snapshot_date.ge(start));
        }
        if let Some(end) = end_date {
            query = query.filter(snapshots_dsl::snapshot_date.le(end));
        }

        query
            .load::<AccountStateSnapshotDB>(&mut conn)
            .map(|rows| rows.into_iter().map(AccountStateSnapshot::from).collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn get_latest_snapshot_before_date(
        &self,
        account_id: &str,
        date: NaiveDate,
    ) -> Result<Option<AccountStateSnapshot>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .filter(snapshots_dsl::snapshot_date.le(date))
            .order(snapshots_dsl::snapshot_date.desc())
            .select(AccountStateSnapshotDB::as_select())
            .first::<AccountStateSnapshotDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(AccountStateSnapshot::from))
    }

    fn get_latest_snapshots_before_date(
        &self,
        account_ids: &[String],
        date: NaiveDate,
    ) -> Result<HashMap<String, AccountStateSnapshot>> {
        let rows = self.latest_snapshot_rows(account_ids, Some(date))?;
        let mut result = HashMap::new();
        for row in rows {
            let account_id = account_id_to_domain(row.account_id);
            result.entry(account_id).or_insert_with(|| AccountStateSnapshot::from(row));
        }
        Ok(result)
    }

    fn get_all_latest_snapshots(
        &self,
        account_ids: &[String],
    ) -> Result<HashMap<String, AccountStateSnapshot>> {
        let rows = self.latest_snapshot_rows(account_ids, None)?;
        let mut result = HashMap::new();
        for row in rows {
            let account_id = account_id_to_domain(row.account_id);
            result.entry(account_id).or_insert_with(|| AccountStateSnapshot::from(row));
        }
        Ok(result)
    }

    async fn delete_snapshots_by_account_ids(&self, account_ids: &[String]) -> Result<usize> {
        if account_ids.is_empty() {
            return Ok(0);
        }

        let parsed_ids = account_ids
            .iter()
            .map(|account_id| parse_account_id(account_id))
            .collect::<Result<Vec<_>>>()?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq_any(parsed_ids))
                .filter(snapshots_dsl::source.eq(SOURCE_CALCULATED)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)
        .map_err(Into::into)
    }

    async fn delete_snapshots_for_account_and_dates(
        &self,
        account_id: &str,
        dates_to_delete: &[NaiveDate],
    ) -> Result<()> {
        if dates_to_delete.is_empty() {
            return Ok(());
        }

        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq(parsed_account_id))
                .filter(snapshots_dsl::snapshot_date.eq_any(dates_to_delete)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;
        Ok(())
    }

    async fn delete_snapshots_for_account_in_range(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<()> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq(parsed_account_id))
                .filter(snapshots_dsl::snapshot_date.ge(start_date))
                .filter(snapshots_dsl::snapshot_date.le(end_date)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;
        Ok(())
    }

    async fn overwrite_snapshots_for_account_in_range(
        &self,
        account_id: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        snapshots_to_save: &[AccountStateSnapshot],
    ) -> Result<()> {
        let parsed_account_id = parse_account_id(account_id)?;
        let anchor_dates = self
            .get_anchor_snapshot_dates_for_account_in_range(account_id, start_date, end_date)
            .await?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq(parsed_account_id))
                .filter(snapshots_dsl::snapshot_date.ge(start_date))
                .filter(snapshots_dsl::snapshot_date.le(end_date))
                .filter(snapshots_dsl::source.eq(SOURCE_CALCULATED)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        let filtered_snapshots: Vec<AccountStateSnapshot> = snapshots_to_save
            .iter()
            .filter(|snapshot| snapshot.account_id == account_id)
            .filter(|snapshot| !anchor_dates.contains(&snapshot.snapshot_date))
            .cloned()
            .collect();

        self.upsert_snapshot_rows(&filtered_snapshots).await
    }

    async fn overwrite_multiple_account_snapshot_ranges(
        &self,
        new_snapshots: &[AccountStateSnapshot],
    ) -> Result<()> {
        if new_snapshots.is_empty() {
            return Ok(());
        }

        let mut snapshots_by_account: HashMap<String, Vec<AccountStateSnapshot>> = HashMap::new();
        for snapshot in new_snapshots {
            snapshots_by_account
                .entry(snapshot.account_id.clone())
                .or_default()
                .push(snapshot.clone());
        }

        for (account_id, snapshots) in snapshots_by_account {
            if let Some(first) = snapshots.first() {
                let mut min_date = first.snapshot_date;
                let mut max_date = first.snapshot_date;
                for snapshot in snapshots.iter().skip(1) {
                    min_date = min_date.min(snapshot.snapshot_date);
                    max_date = max_date.max(snapshot.snapshot_date);
                }
                self.overwrite_snapshots_for_account_in_range(
                    &account_id,
                    min_date,
                    max_date,
                    &snapshots,
                )
                .await?;
            }
        }

        Ok(())
    }

    fn get_total_portfolio_snapshots(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<AccountStateSnapshot>> {
        self.get_snapshots_by_account(PORTFOLIO_TOTAL_ACCOUNT_ID, start_date, end_date)
    }

    fn get_all_non_archived_account_snapshots(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<AccountStateSnapshot>> {
        let mut conn = get_connection(&self.pool)?;
        let non_archived_account_ids = accounts_dsl::wf_accounts
            .filter(accounts_dsl::is_archived.eq(false))
            .select(accounts_dsl::id)
            .load::<uuid::Uuid>(&mut conn)
            .map_err(StorageError::from)?;

        if non_archived_account_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.ne(total_portfolio_account_uuid()))
            .filter(snapshots_dsl::account_id.eq_any(non_archived_account_ids))
            .order(snapshots_dsl::snapshot_date.asc())
            .select(AccountStateSnapshotDB::as_select())
            .into_boxed();

        if let Some(start) = start_date {
            query = query.filter(snapshots_dsl::snapshot_date.ge(start));
        }
        if let Some(end) = end_date {
            query = query.filter(snapshots_dsl::snapshot_date.le(end));
        }

        query
            .load::<AccountStateSnapshotDB>(&mut conn)
            .map(|rows| rows.into_iter().map(AccountStateSnapshot::from).collect())
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn get_earliest_snapshot_date(&self, account_id: &str) -> Result<Option<NaiveDate>> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        let result = snapshots_dsl::wf_holdings_snapshots
            .filter(snapshots_dsl::account_id.eq(parsed_account_id))
            .select(snapshots_dsl::snapshot_date)
            .order(snapshots_dsl::snapshot_date.asc())
            .first::<NaiveDate>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result)
    }

    async fn overwrite_all_snapshots_for_account(
        &self,
        account_id: &str,
        snapshots_to_save: &[AccountStateSnapshot],
    ) -> Result<()> {
        let parsed_account_id = parse_account_id(account_id)?;
        let anchor_dates = self.get_anchor_snapshot_dates_for_account(account_id).await?;
        let filtered_snapshots: Vec<AccountStateSnapshot> = snapshots_to_save
            .iter()
            .filter(|snapshot| snapshot.account_id == account_id)
            .filter(|snapshot| !anchor_dates.contains(&snapshot.snapshot_date))
            .cloned()
            .collect();
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq(parsed_account_id))
                .filter(snapshots_dsl::source.eq(SOURCE_CALCULATED)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)?;

        self.upsert_snapshot_rows(&filtered_snapshots).await
    }

    async fn update_snapshots_source(&self, account_id: &str, new_source: &str) -> Result<usize> {
        let parsed_account_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;
        diesel::update(
            snapshots_dsl::wf_holdings_snapshots
                .filter(snapshots_dsl::account_id.eq(parsed_account_id)),
        )
        .set(snapshots_dsl::source.eq(new_source))
        .execute(&mut conn)
        .map_err(StorageError::from)
        .map_err(Into::into)
    }

    async fn save_or_update_snapshot(&self, snapshot: &AccountStateSnapshot) -> Result<()> {
        self.save_or_update_snapshot_impl(snapshot).await
    }

    fn get_non_calculated_snapshot_count(&self, account_id: &str) -> Result<usize> {
        self.get_non_calculated_snapshot_count_impl(account_id)
    }

    fn get_earliest_non_calculated_snapshot(
        &self,
        account_id: &str,
    ) -> Result<Option<AccountStateSnapshot>> {
        self.get_earliest_non_calculated_snapshot_impl(account_id)
    }
}