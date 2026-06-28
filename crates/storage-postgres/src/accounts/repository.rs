use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;

use crate::accounts::model::AccountDB;
use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_accounts;
use crate::schema::wf_accounts::dsl::*;
use crate::system_accounts::{parse_account_id, total_portfolio_account_uuid};
use wealthfolio_core::accounts::{Account, AccountRepositoryTrait, AccountUpdate, NewAccount};
use wealthfolio_core::errors::Result;

pub struct AccountRepository {
    pool: Arc<DbPool>,
}

impl AccountRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountRepositoryTrait for AccountRepository {
    async fn create(&self, new_account: NewAccount) -> Result<Account> {
        new_account.validate()?;

        let account_db = AccountDB::from_new_account(new_account)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(wf_accounts::table)
            .values(&account_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(account_db.into())
    }

    async fn update(&self, account_update: AccountUpdate) -> Result<Account> {
        account_update.validate()?;

        let is_archived_provided = account_update.is_archived.is_some();
        let tracking_mode_provided = account_update.tracking_mode.is_some();
        let mut account_db = AccountDB::from_account_update(account_update)?;
        let mut conn = get_connection(&self.pool)?;

        let existing = wf_accounts
            .select(AccountDB::as_select())
            .find(account_db.id)
            .first::<AccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        account_db.currency = existing.currency;
        account_db.created_at = existing.created_at;
        account_db.updated_at = chrono::Utc::now();
        account_db.provider_account_id = existing.provider_account_id;
        account_db.platform_id = existing.platform_id;
        account_db.provider = existing.provider;
        account_db.account_number = existing.account_number;
        account_db.meta = existing.meta;

        if !is_archived_provided {
            account_db.is_archived = existing.is_archived;
        }
        if !tracking_mode_provided {
            account_db.tracking_mode = existing.tracking_mode;
        }

        diesel::update(wf_accounts.find(account_db.id))
            .set(&account_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(account_db.into())
    }

    async fn delete(&self, account_id_param: &str) -> Result<usize> {
        let parsed_id = parse_account_id(account_id_param)?;
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(wf_accounts.find(parsed_id))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn get_by_id(&self, account_id: &str) -> Result<Account> {
        let parsed_id = parse_account_id(account_id)?;
        let mut conn = get_connection(&self.pool)?;

        let account = wf_accounts
            .select(AccountDB::as_select())
            .find(parsed_id)
            .first::<AccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(account.into())
    }

    fn list(
        &self,
        is_active_filter: Option<bool>,
        is_archived_filter: Option<bool>,
        account_ids: Option<&[String]>,
    ) -> Result<Vec<Account>> {
        let mut conn = get_connection(&self.pool)?;
        let mut query = wf_accounts::table
            .filter(id.ne(total_portfolio_account_uuid()))
            .into_boxed();

        if let Some(active) = is_active_filter {
            query = query.filter(is_active.eq(active));
        }

        if let Some(archived) = is_archived_filter {
            query = query.filter(is_archived.eq(archived));
        }

        if let Some(ids) = account_ids {
            let parsed_ids = ids
                .iter()
                .map(|value| parse_account_id(value))
                .collect::<Result<Vec<_>>>()?;
            query = wf_accounts::table.filter(id.eq_any(parsed_ids)).into_boxed();

            if let Some(active) = is_active_filter {
                query = query.filter(is_active.eq(active));
            }

            if let Some(archived) = is_archived_filter {
                query = query.filter(is_archived.eq(archived));
            }
        }

        let results = query
            .select(AccountDB::as_select())
            .order((is_active.desc(), is_archived.asc(), name.asc()))
            .load::<AccountDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Account::from).collect())
    }
}