use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_app_settings::dsl::*;
use crate::schema::{wf_accounts, wf_assets};
use crate::settings::model::AppSettingDB;
use wealthfolio_core::assets::AssetKind;
use wealthfolio_core::errors::Result;
use wealthfolio_core::settings::{Settings, SettingsRepositoryTrait, SettingsUpdate};

pub struct SettingsRepository {
    pool: Arc<DbPool>,
}

impl SettingsRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    fn upsert_setting(
        conn: &mut diesel::PgConnection,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let record = AppSettingDB {
            setting_key: key.to_string(),
            setting_value: value.to_string(),
        };

        diesel::insert_into(crate::schema::wf_app_settings::table)
            .values(&record)
            .on_conflict(setting_key)
            .do_update()
            .set(setting_value.eq(value))
            .execute(conn)
            .map_err(StorageError::from)?;

        Ok(())
    }
}

#[async_trait]
impl SettingsRepositoryTrait for SettingsRepository {
    fn get_settings(&self) -> Result<Settings> {
        let mut conn = get_connection(&self.pool)?;
        let all_settings: Vec<(String, String)> = crate::schema::wf_app_settings::table
            .select((setting_key, setting_value))
            .load::<(String, String)>(&mut conn)
            .map_err(StorageError::from)?;

        let mut settings = Settings::default();

        for (key, value) in all_settings {
            match key.as_str() {
                "theme" => settings.theme = value,
                "font" => settings.font = value,
                "base_currency" => settings.base_currency = value,
                "timezone" => settings.timezone = value,
                "instance_id" => settings.instance_id = value,
                "onboarding_completed" => {
                    settings.onboarding_completed = value.parse().unwrap_or(false)
                }
                "auto_update_check_enabled" => {
                    settings.auto_update_check_enabled = value.parse().unwrap_or(true)
                }
                "menu_bar_visible" => settings.menu_bar_visible = value.parse().unwrap_or(true),
                "sync_enabled" => settings.sync_enabled = value.parse().unwrap_or(true),
                _ => {}
            }
        }

        Ok(settings)
    }

    async fn update_settings(&self, new_settings: &SettingsUpdate) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;

        if let Some(ref value) = new_settings.theme {
            Self::upsert_setting(&mut conn, "theme", value)?;
        }
        if let Some(ref value) = new_settings.font {
            Self::upsert_setting(&mut conn, "font", value)?;
        }
        if let Some(ref value) = new_settings.base_currency {
            Self::upsert_setting(&mut conn, "base_currency", value)?;
        }
        if let Some(ref value) = new_settings.timezone {
            Self::upsert_setting(&mut conn, "timezone", value)?;
        }
        if let Some(value) = new_settings.onboarding_completed {
            Self::upsert_setting(&mut conn, "onboarding_completed", &value.to_string())?;
        }
        if let Some(value) = new_settings.auto_update_check_enabled {
            Self::upsert_setting(&mut conn, "auto_update_check_enabled", &value.to_string())?;
        }
        if let Some(value) = new_settings.menu_bar_visible {
            Self::upsert_setting(&mut conn, "menu_bar_visible", &value.to_string())?;
        }
        if let Some(value) = new_settings.sync_enabled {
            Self::upsert_setting(&mut conn, "sync_enabled", &value.to_string())?;
        }

        Ok(())
    }

    fn get_setting(&self, setting_key_param: &str) -> Result<String> {
        let mut conn = get_connection(&self.pool)?;
        let result = crate::schema::wf_app_settings::table
            .filter(setting_key.eq(setting_key_param))
            .select(setting_value)
            .first::<String>(&mut conn);

        match result {
            Ok(value) => Ok(value),
            Err(diesel::result::Error::NotFound) => {
                let default_value = match setting_key_param {
                    "theme" => "light",
                    "font" => "font-mono",
                    "timezone" => "",
                    "onboarding_completed" => "false",
                    "auto_update_check_enabled" => "true",
                    "menu_bar_visible" => "true",
                    "sync_enabled" => "true",
                    _ => return Err(StorageError::from(diesel::result::Error::NotFound).into()),
                };
                Ok(default_value.to_string())
            }
            Err(err) => Err(StorageError::from(err).into()),
        }
    }

    async fn update_setting(&self, setting_key_param: &str, setting_value_param: &str) -> Result<()> {
        let mut conn = get_connection(&self.pool)?;
        Self::upsert_setting(&mut conn, setting_key_param, setting_value_param)
    }

    fn get_distinct_currencies_excluding_base(&self, base_currency_param: &str) -> Result<Vec<String>> {
        let mut conn = get_connection(&self.pool)?;

        let currency_assets: Vec<String> = wf_assets::table
            .filter(wf_assets::kind.eq(AssetKind::Fx.as_db_str()))
            .filter(wf_assets::quote_ccy.ne(base_currency_param))
            .select(wf_assets::quote_ccy)
            .distinct()
            .load::<String>(&mut conn)
            .map_err(StorageError::from)?;

        let account_currencies: Vec<String> = wf_accounts::table
            .filter(wf_accounts::currency.ne(base_currency_param))
            .select(wf_accounts::currency)
            .distinct()
            .load::<String>(&mut conn)
            .map_err(StorageError::from)?;

        let mut all_currencies = Vec::new();
        all_currencies.extend(currency_assets);
        all_currencies.extend(account_currencies);
        all_currencies.sort();
        all_currencies.dedup();

        Ok(all_currencies)
    }
}