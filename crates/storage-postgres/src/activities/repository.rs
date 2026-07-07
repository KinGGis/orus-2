use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use diesel::dsl::{max, min};
use diesel::prelude::*;
use diesel::sql_types::Text;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::activities::model::{
    apply_decimal_patch, format_activity_date, format_activity_timestamp, ActivityDB,
    ImportMappingDB,
};
use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{wf_accounts, wf_activities, wf_activity_import_profiles, wf_assets};
use wealthfolio_core::activities::ActivityError;
use wealthfolio_core::activities::{
    Activity, ActivityBulkIdentifierMapping, ActivityBulkMutationResult, ActivityDetails,
    ActivityRepositoryTrait, ActivitySearchResponse, ActivitySearchResponseMeta, ActivityUpdate,
    ActivityUpsert, BulkUpsertResult, ImportMapping, IncomeData, NewActivity, Sort,
    INCOME_ACTIVITY_TYPES, TRADING_ACTIVITY_TYPES,
};
use wealthfolio_core::errors::{Result, ValidationError};
use wealthfolio_core::limits::ContributionActivity;
use wealthfolio_core::Error;

pub struct ActivityRepository {
    pool: Arc<DbPool>,
}

impl ActivityRepository {
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

    fn parse_uuid_vec(values: &[String], field: &str) -> Result<Vec<Uuid>> {
        values
            .iter()
            .map(|value| Self::parse_uuid(value, field))
            .collect()
    }
}

#[async_trait]
impl ActivityRepositoryTrait for ActivityRepository {
    fn get_activity(&self, activity_id: &str) -> Result<Activity> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_id = Self::parse_uuid(activity_id, "activity_id")?;
        let activity_db = wf_activities::table
            .select(ActivityDB::as_select())
            .find(parsed_id)
            .first::<ActivityDB>(&mut conn)
            .map_err(|e| Error::from(ActivityError::NotFound(e.to_string())))?;
        Ok(Activity::from(activity_db))
    }

    fn get_trading_activities(&self) -> Result<Vec<Activity>> {
        let mut conn = get_connection(&self.pool)?;
        let activities_db = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_accounts::id.eq(wf_activities::account_id)))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::activity_type.eq_any(TRADING_ACTIVITY_TYPES))
            .select(ActivityDB::as_select())
            .order(wf_activities::activity_date.asc())
            .load::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activities_db.into_iter().map(Activity::from).collect())
    }

    fn get_income_activities(&self) -> Result<Vec<Activity>> {
        let mut conn = get_connection(&self.pool)?;
        let activities_db = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_accounts::id.eq(wf_activities::account_id)))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::activity_type.eq_any(INCOME_ACTIVITY_TYPES))
            .select(ActivityDB::as_select())
            .order(wf_activities::activity_date.asc())
            .load::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activities_db.into_iter().map(Activity::from).collect())
    }

    fn get_activities(&self) -> Result<Vec<Activity>> {
        let mut conn = get_connection(&self.pool)?;
        let activities_db = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_accounts::id.eq(wf_activities::account_id)))
            .filter(wf_accounts::is_archived.eq(false))
            .select(ActivityDB::as_select())
            .order(wf_activities::activity_date.asc())
            .load::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activities_db.into_iter().map(Activity::from).collect())
    }

    fn search_activities(
        &self,
        page: i64,
        page_size: i64,
        account_id_filter: Option<Vec<String>>,
        activity_type_filter: Option<Vec<String>>,
        asset_id_keyword: Option<String>,
        sort: Option<Sort>,
        needs_review_filter: Option<bool>,
        date_from: Option<NaiveDate>,
        date_to: Option<NaiveDate>,
        instrument_type_filter: Option<Vec<String>>,
    ) -> Result<ActivitySearchResponse> {
        let mut conn = get_connection(&self.pool)?;
        let offset = page * page_size;

        let create_base_query = || {
            let mut query = wf_activities::table
                .inner_join(wf_accounts::table.on(wf_activities::account_id.eq(wf_accounts::id)))
                .left_join(wf_assets::table.on(wf_activities::asset_id.eq(wf_assets::id.nullable())))
                .filter(wf_accounts::is_archived.eq(false))
                .into_boxed();

            if let Some(ref account_ids) = account_id_filter {
                if let Ok(parsed) = Self::parse_uuid_vec(account_ids, "account_id") {
                    query = query.filter(wf_activities::account_id.eq_any(parsed));
                }
            }
            if let Some(ref activity_types) = activity_type_filter {
                query = query.filter(wf_activities::activity_type.eq_any(activity_types));
            }
            if let Some(ref keyword) = asset_id_keyword {
                let escaped = keyword.replace('\'', "''");
                let pattern = format!("%{}%", escaped.to_uppercase());
                query = query.filter(diesel::dsl::sql::<diesel::sql_types::Bool>(&format!(
                    "UPPER(CAST(wf_assets.id AS TEXT)) LIKE '{}' OR UPPER(COALESCE(wf_assets.name, '')) LIKE '{}' OR UPPER(COALESCE(wf_assets.display_code, '')) LIKE '{}' OR UPPER(COALESCE(wf_activities.notes, '')) LIKE '{}'",
                    pattern, pattern, pattern, pattern
                )));
            }
            if let Some(needs_review) = needs_review_filter {
                query = if needs_review {
                    query.filter(wf_activities::status.eq("DRAFT"))
                } else {
                    query.filter(wf_activities::status.ne("DRAFT"))
                };
            }
            if let Some(from_date) = date_from {
                query = query.filter(wf_activities::activity_date.ge(from_date));
            }
            if let Some(to_date) = date_to {
                query = query.filter(wf_activities::activity_date.le(to_date));
            }
            if let Some(ref instrument_types) = instrument_type_filter {
                query = query.filter(wf_assets::instrument_type.eq_any(instrument_types));
            }

            query
        };

        // NOTE: the ordering must NOT be applied to the base query, because the
        // same builder is reused for the COUNT(*) query. In Postgres an
        // `ORDER BY <non-aggregated column>` combined with an aggregate select
        // triggers "column ... must appear in the GROUP BY clause". The sort is
        // therefore applied only to the data-fetching query below.
        let total_row_count = create_base_query()
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(StorageError::from)?;

        type SearchRow = (
            Uuid,
            Uuid,
            Option<Uuid>,
            String,
            Option<String>,
            String,
            NaiveDate,
            Option<bigdecimal::BigDecimal>,
            Option<bigdecimal::BigDecimal>,
            String,
            Option<bigdecimal::BigDecimal>,
            Option<bigdecimal::BigDecimal>,
            Option<String>,
            Option<bigdecimal::BigDecimal>,
            bool,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<Uuid>,
            DateTime<Utc>,
            DateTime<Utc>,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<serde_json::Value>,
        );

        let sorted_query = {
            let mut query = create_base_query();
            if let Some(ref sort) = sort {
                match sort.id.as_str() {
                    "date" => {
                        if sort.desc {
                            query = query.order((wf_activities::activity_date.desc(), wf_activities::created_at.asc()));
                        } else {
                            query = query.order((wf_activities::activity_date.asc(), wf_activities::created_at.asc()));
                        }
                    }
                    "activityType" => {
                        query = if sort.desc {
                            query.order(wf_activities::activity_type.desc())
                        } else {
                            query.order(wf_activities::activity_type.asc())
                        };
                    }
                    "assetSymbol" => {
                        query = if sort.desc {
                            query.order(wf_activities::asset_id.desc())
                        } else {
                            query.order(wf_activities::asset_id.asc())
                        };
                    }
                    "accountName" => {
                        query = if sort.desc {
                            query.order(wf_accounts::name.desc())
                        } else {
                            query.order(wf_accounts::name.asc())
                        };
                    }
                    _ => {
                        query = query.order((wf_activities::activity_date.desc(), wf_activities::created_at.asc()));
                    }
                }
            } else {
                query = query.order((wf_activities::activity_date.desc(), wf_activities::created_at.asc()));
            }
            query
        };

        let rows = sorted_query
            .select((
                wf_activities::id,
                wf_activities::account_id,
                wf_activities::asset_id,
                wf_activities::activity_type,
                wf_activities::subtype,
                wf_activities::status,
                wf_activities::activity_date,
                wf_activities::quantity,
                wf_activities::unit_price,
                wf_activities::currency,
                wf_activities::fee,
                wf_activities::amount,
                wf_activities::notes,
                wf_activities::fx_rate,
                wf_activities::needs_review,
                wf_activities::is_user_modified,
                wf_activities::source_system,
                wf_activities::source_record_id,
                wf_activities::idempotency_key,
                wf_activities::import_run_id,
                wf_activities::created_at,
                wf_activities::updated_at,
                wf_accounts::name,
                wf_accounts::currency,
                wf_assets::display_code.nullable(),
                wf_assets::name.nullable(),
                wf_assets::instrument_exchange_mic.nullable(),
                wf_assets::quote_mode.nullable(),
                wf_assets::instrument_type.nullable(),
                wf_activities::metadata,
            ))
            .limit(page_size)
            .offset(offset)
            .load::<SearchRow>(&mut conn)
            .map_err(StorageError::from)?;

        let data = rows
            .into_iter()
            .map(|row| ActivityDetails {
                id: row.0.to_string(),
                account_id: row.1.to_string(),
                asset_id: row.2.map(|value| value.to_string()).unwrap_or_default(),
                activity_type: row.3,
                subtype: row.4,
                status: match row.5.as_str() {
                    "POSTED" => wealthfolio_core::activities::ActivityStatus::Posted,
                    "PENDING" => wealthfolio_core::activities::ActivityStatus::Pending,
                    "DRAFT" => wealthfolio_core::activities::ActivityStatus::Draft,
                    "VOID" => wealthfolio_core::activities::ActivityStatus::Void,
                    _ => wealthfolio_core::activities::ActivityStatus::Posted,
                },
                date: format_activity_date(row.6),
                quantity: row.7.map(|v| v.to_string()),
                unit_price: row.8.map(|v| v.to_string()),
                currency: row.9,
                fee: row.10.map(|v| v.to_string()),
                amount: row.11.map(|v| v.to_string()),
                needs_review: row.14,
                comment: row.12,
                fx_rate: row.13.map(|v| v.to_string()),
                created_at: format_activity_timestamp(row.20),
                updated_at: format_activity_timestamp(row.21),
                account_name: row.22,
                account_currency: row.23,
                asset_symbol: row.24.unwrap_or_default(),
                asset_name: row.25,
                exchange_mic: row.26,
                asset_pricing_mode: row.27.unwrap_or_else(|| "MARKET".to_string()),
                instrument_type: row.28,
                source_system: row.16,
                source_record_id: row.17,
                idempotency_key: row.18,
                import_run_id: row.19.map(|value| value.to_string()),
                is_user_modified: row.15,
                metadata: row.29,
            })
            .collect();

        Ok(ActivitySearchResponse {
            data,
            meta: ActivitySearchResponseMeta { total_row_count },
        })
    }

    async fn create_activity(&self, new_activity: NewActivity) -> Result<Activity> {
        new_activity.validate()?;
        let mut activity_db = ActivityDB::from_new_activity(new_activity)?;
        activity_db.id = Uuid::new_v4();
        let mut conn = get_connection(&self.pool)?;
        let inserted_activity = diesel::insert_into(wf_activities::table)
            .values(&activity_db)
            .get_result::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(inserted_activity.into())
    }

    async fn update_activity(&self, activity_update: ActivityUpdate) -> Result<Activity> {
        activity_update.validate()?;
        let activity_update_owned = activity_update.clone();
        let mut activity_to_update = ActivityDB::from_activity_update(activity_update)?;
        let mut conn = get_connection(&self.pool)?;

        let existing = wf_activities::table
            .select(ActivityDB::as_select())
            .find(activity_to_update.id)
            .first::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;

        let ActivityDB {
            created_at,
            fx_rate,
            source_system,
            source_record_id,
            source_group_id,
            idempotency_key,
            import_run_id,
            activity_type_override,
            source_type,
            subtype,
            settlement_date,
            metadata,
            quantity,
            unit_price,
            amount,
            fee,
            ..
        } = existing;

        activity_to_update.created_at = created_at;
        activity_to_update.quantity = apply_decimal_patch(quantity, activity_update_owned.quantity);
        activity_to_update.unit_price =
            apply_decimal_patch(unit_price, activity_update_owned.unit_price);
        activity_to_update.amount = apply_decimal_patch(amount, activity_update_owned.amount);
        activity_to_update.fee = apply_decimal_patch(fee, activity_update_owned.fee);
        activity_to_update.fx_rate = apply_decimal_patch(fx_rate, activity_update_owned.fx_rate);
        if activity_to_update.source_system.is_none() {
            activity_to_update.source_system = source_system;
        }
        if activity_to_update.source_record_id.is_none() {
            activity_to_update.source_record_id = source_record_id;
        }
        if activity_to_update.source_group_id.is_none() {
            activity_to_update.source_group_id = source_group_id;
        }
        if activity_to_update.idempotency_key.is_none() {
            activity_to_update.idempotency_key = idempotency_key;
        }
        if activity_to_update.import_run_id.is_none() {
            activity_to_update.import_run_id = import_run_id;
        }
        if activity_to_update.activity_type_override.is_none() {
            activity_to_update.activity_type_override = activity_type_override;
        }
        if activity_to_update.source_type.is_none() {
            activity_to_update.source_type = source_type;
        }
        if activity_to_update.subtype.is_none() {
            activity_to_update.subtype = subtype;
        }
        if activity_to_update.settlement_date.is_none() {
            activity_to_update.settlement_date = settlement_date;
        }
        if activity_to_update.metadata.is_none() {
            activity_to_update.metadata = metadata;
        }
        activity_to_update.updated_at = Utc::now();

        let updated_activity = diesel::update(wf_activities::table.find(activity_to_update.id))
            .set(&activity_to_update)
            .get_result::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(updated_activity.into())
    }

    async fn delete_activity(&self, activity_id: String) -> Result<Activity> {
        let parsed_id = Self::parse_uuid(&activity_id, "activity_id")?;
        let mut conn = get_connection(&self.pool)?;
        let activity = wf_activities::table
            .select(ActivityDB::as_select())
            .find(parsed_id)
            .first::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        diesel::delete(wf_activities::table.filter(wf_activities::id.eq(parsed_id)))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activity.into())
    }

    async fn bulk_mutate_activities(
        &self,
        creates: Vec<NewActivity>,
        updates: Vec<ActivityUpdate>,
        delete_ids: Vec<String>,
    ) -> Result<ActivityBulkMutationResult> {
        let mut conn = get_connection(&self.pool)?;
        let mut outcome = ActivityBulkMutationResult::default();

        for delete_id in delete_ids {
            let parsed_id = Self::parse_uuid(&delete_id, "activity_id")?;
            let activity_db = wf_activities::table
                .select(ActivityDB::as_select())
                .find(parsed_id)
                .first::<ActivityDB>(&mut conn)
                .map_err(StorageError::from)?;
            diesel::delete(wf_activities::table.filter(wf_activities::id.eq(parsed_id)))
                .execute(&mut conn)
                .map_err(StorageError::from)?;
            outcome.deleted.push(Activity::from(activity_db));
        }

        for update in updates {
            update.validate()?;
            let update_owned = update.clone();
            let mut activity_db = ActivityDB::from_activity_update(update)?;
            let existing = wf_activities::table
                .select(ActivityDB::as_select())
                .find(activity_db.id)
                .first::<ActivityDB>(&mut conn)
                .map_err(StorageError::from)?;

            let ActivityDB {
                created_at,
                source_system,
                source_record_id,
                source_group_id,
                idempotency_key,
                import_run_id,
                activity_type_override,
                source_type,
                subtype,
                settlement_date,
                metadata,
                quantity,
                unit_price,
                amount,
                fee,
                fx_rate,
                ..
            } = existing;

            activity_db.created_at = created_at;
            activity_db.quantity = apply_decimal_patch(quantity, update_owned.quantity);
            activity_db.unit_price = apply_decimal_patch(unit_price, update_owned.unit_price);
            activity_db.amount = apply_decimal_patch(amount, update_owned.amount);
            activity_db.fee = apply_decimal_patch(fee, update_owned.fee);
            activity_db.fx_rate = apply_decimal_patch(fx_rate, update_owned.fx_rate);
            if activity_db.source_system.is_none() {
                activity_db.source_system = source_system;
            }
            if activity_db.source_record_id.is_none() {
                activity_db.source_record_id = source_record_id;
            }
            if activity_db.source_group_id.is_none() {
                activity_db.source_group_id = source_group_id;
            }
            if activity_db.idempotency_key.is_none() {
                activity_db.idempotency_key = idempotency_key;
            }
            if activity_db.import_run_id.is_none() {
                activity_db.import_run_id = import_run_id;
            }
            if activity_db.activity_type_override.is_none() {
                activity_db.activity_type_override = activity_type_override;
            }
            if activity_db.source_type.is_none() {
                activity_db.source_type = source_type;
            }
            if activity_db.subtype.is_none() {
                activity_db.subtype = subtype;
            }
            if activity_db.settlement_date.is_none() {
                activity_db.settlement_date = settlement_date;
            }
            if activity_db.metadata.is_none() {
                activity_db.metadata = metadata;
            }
            activity_db.updated_at = Utc::now();

            let updated_activity = diesel::update(wf_activities::table.find(activity_db.id))
                .set(&activity_db)
                .get_result::<ActivityDB>(&mut conn)
                .map_err(StorageError::from)?;
            outcome.updated.push(Activity::from(updated_activity));
        }

        for new_activity in creates {
            new_activity.validate()?;
            let temp_id = new_activity.id.clone();
            let mut activity_db = ActivityDB::from_new_activity(new_activity)?;
            let generated_id = Uuid::new_v4();
            activity_db.id = generated_id;
            let inserted_activity = diesel::insert_into(wf_activities::table)
                .values(&activity_db)
                .get_result::<ActivityDB>(&mut conn)
                .map_err(StorageError::from)?;
            outcome.created.push(Activity::from(inserted_activity));
            outcome.created_mappings.push(ActivityBulkIdentifierMapping {
                temp_id: temp_id.filter(|id| !id.is_empty()),
                activity_id: generated_id.to_string(),
            });
        }

        Ok(outcome)
    }

    fn get_activities_by_account_id(&self, account_id: &str) -> Result<Vec<Activity>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_account_id = Self::parse_uuid(account_id, "account_id")?;
        let activities_db = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_accounts::id.eq(wf_activities::account_id)))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::account_id.eq(parsed_account_id))
            .select(ActivityDB::as_select())
            .order(wf_activities::activity_date.asc())
            .load::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activities_db.into_iter().map(Activity::from).collect())
    }

    fn get_activities_by_account_ids(&self, account_ids: &[String]) -> Result<Vec<Activity>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_account_ids = Self::parse_uuid_vec(account_ids, "account_id")?;
        let activities_db = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_activities::account_id.eq(wf_accounts::id)))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::account_id.eq_any(parsed_account_ids))
            .select(ActivityDB::as_select())
            .order(wf_activities::activity_date.asc())
            .load::<ActivityDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(activities_db.into_iter().map(Activity::from).collect())
    }

    fn calculate_average_cost(&self, account_id: &str, asset_id: &str) -> Result<Decimal> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_account_id = Self::parse_uuid(account_id, "account_id")?;
        let parsed_asset_id = Self::parse_uuid(asset_id, "asset_id")?;
        let rows = wf_activities::table
            .filter(wf_activities::account_id.eq(parsed_account_id))
            .filter(wf_activities::asset_id.eq(Some(parsed_asset_id)))
            .filter(wf_activities::activity_type.eq_any(["BUY", "TRANSFER_IN"]))
            .select((wf_activities::quantity, wf_activities::unit_price))
            .load::<(Option<bigdecimal::BigDecimal>, Option<bigdecimal::BigDecimal>)>(&mut conn)
            .map_err(StorageError::from)?;

        let mut total_quantity = Decimal::ZERO;
        let mut total_value = Decimal::ZERO;
        for (quantity, unit_price) in rows {
            let qty = quantity
                .as_ref()
                .map(|value| Decimal::from_str(&value.to_string()).unwrap_or_default())
                .unwrap_or_default();
            let price = unit_price
                .as_ref()
                .map(|value| Decimal::from_str(&value.to_string()).unwrap_or_default())
                .unwrap_or_default();
            total_quantity += qty;
            total_value += qty * price;
        }

        if total_quantity.is_zero() {
            Ok(Decimal::ZERO)
        } else {
            Ok(total_value / total_quantity)
        }
    }

    fn get_import_mapping(&self, some_account_id: &str) -> Result<Option<ImportMapping>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_account_id = Self::parse_uuid(some_account_id, "account_id")?;
        let result = wf_activity_import_profiles::table
            .filter(wf_activity_import_profiles::account_id.eq(parsed_account_id))
            .first::<ImportMappingDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(ImportMapping::from))
    }

    async fn save_import_mapping(&self, mapping: &ImportMapping) -> Result<()> {
        let mapping_db: ImportMappingDB = mapping.clone().try_into()?;
        let mut conn = get_connection(&self.pool)?;
        diesel::insert_into(wf_activity_import_profiles::table)
            .values(&mapping_db)
            .on_conflict(wf_activity_import_profiles::account_id)
            .do_update()
            .set(&mapping_db)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn create_activities(&self, activities_vec: Vec<NewActivity>) -> Result<usize> {
        if activities_vec.is_empty() {
            return Ok(0);
        }
        for new_act in &activities_vec {
            new_act.validate()?;
        }

        let activities_db_owned: Vec<ActivityDB> = activities_vec
            .into_iter()
            .map(ActivityDB::from_new_activity)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|mut db| {
                db.id = Uuid::new_v4();
                db
            })
            .collect();

        let mut conn = get_connection(&self.pool)?;
        let num_inserted = diesel::insert_into(wf_activities::table)
            .values(&activities_db_owned)
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(num_inserted)
    }

    fn get_contribution_activities(
        &self,
        account_ids: &[String],
        start_utc: DateTime<Utc>,
        end_exclusive_utc: DateTime<Utc>,
    ) -> Result<Vec<ContributionActivity>> {
        let mut conn = get_connection(&self.pool)?;
        let parsed_account_ids = Self::parse_uuid_vec(account_ids, "account_id")?;
        const CONTRIBUTION_TYPES: [&str; 4] = ["DEPOSIT", "TRANSFER_IN", "TRANSFER_OUT", "CREDIT"];

        let results = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_activities::account_id.eq(wf_accounts::id)))
            .filter(wf_accounts::id.eq_any(parsed_account_ids))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::activity_type.eq_any(CONTRIBUTION_TYPES))
            .filter(wf_activities::activity_date.ge(start_utc.date_naive()))
            .filter(wf_activities::activity_date.lt(end_exclusive_utc.date_naive()))
            .select((
                wf_activities::account_id,
                wf_activities::activity_type,
                wf_activities::activity_date,
                wf_activities::amount,
                wf_activities::currency,
                wf_activities::metadata,
                wf_activities::source_group_id,
            ))
            .load::<(
                Uuid,
                String,
                NaiveDate,
                Option<bigdecimal::BigDecimal>,
                String,
                Option<serde_json::Value>,
                Option<String>,
            )>(&mut conn)
            .map_err(ActivityError::from)?;

        Ok(results
            .into_iter()
            .map(|(account_id, activity_type, activity_date, amount, currency, metadata, source_group_id)| ContributionActivity {
                account_id: account_id.to_string(),
                activity_type,
                activity_instant: Utc.from_utc_datetime(&activity_date.and_hms_opt(0, 0, 0).unwrap_or_default()),
                amount: amount.map(|value| Decimal::from_str(&value.to_string()).unwrap_or_default()),
                currency,
                metadata: metadata.map(|value| value.to_string()),
                source_group_id,
            })
            .collect())
    }

    fn get_income_activities_data(&self) -> Result<Vec<IncomeData>> {
        #[derive(QueryableByName, Debug)]
        struct RawIncomeData {
            #[diesel(sql_type = Text)]
            date: String,
            #[diesel(sql_type = Text)]
            income_type: String,
            #[diesel(sql_type = Text)]
            asset_id: String,
            #[diesel(sql_type = Text)]
            asset_kind: String,
            #[diesel(sql_type = Text)]
            symbol: String,
            #[diesel(sql_type = Text)]
            symbol_name: String,
            #[diesel(sql_type = Text)]
            currency: String,
            #[diesel(sql_type = Text)]
            amount: String,
        }

        let mut conn = get_connection(&self.pool)?;
        let query = "SELECT to_char(a.activity_date, 'YYYY-MM') as date,
             a.activity_type as income_type,
             COALESCE(CAST(a.asset_id AS TEXT), 'CASH') as asset_id,
             COALESCE(ast.kind, 'CASH') as asset_kind,
             COALESCE(ast.display_code, 'CASH') as symbol,
             COALESCE(ast.name, 'Cash') as symbol_name,
             a.currency,
             CASE
                 WHEN a.subtype IN ('STAKING_REWARD', 'DRIP', 'DIVIDEND_IN_KIND')
                      AND (a.amount IS NULL OR a.amount = 0)
                 THEN CASE
                     WHEN a.unit_price IS NOT NULL AND a.unit_price > 0
                     THEN (a.quantity * a.unit_price)::text
                     WHEN q.close IS NOT NULL
                     THEN (a.quantity * q.close)::text
                     ELSE '0'
                 END
                 ELSE COALESCE(a.amount::text, '0')
             END as amount
             FROM wf_activities a
             LEFT JOIN wf_assets ast ON a.asset_id = ast.id
             INNER JOIN wf_accounts acc ON a.account_id = acc.id
             LEFT JOIN wf_quotes q ON a.asset_id = q.asset_id
                 AND a.activity_date = q.day
             WHERE a.activity_type IN ('DIVIDEND', 'INTEREST', 'OTHER_INCOME')
             AND acc.is_archived = false
             ORDER BY a.activity_date";

        let raw_results = diesel::sql_query(query)
            .load::<RawIncomeData>(&mut conn)
            .map_err(ActivityError::from)?;

        raw_results
            .into_iter()
            .map(|raw| {
                Ok(IncomeData {
                    date: raw.date,
                    income_type: raw.income_type,
                    asset_id: raw.asset_id,
                    asset_kind: raw.asset_kind,
                    symbol: raw.symbol,
                    symbol_name: raw.symbol_name,
                    currency: raw.currency,
                    amount: Decimal::from_str(&raw.amount).unwrap_or_default(),
                })
            })
            .collect()
    }

    fn get_first_activity_date_overall(&self) -> Result<DateTime<Utc>> {
        let mut conn = get_connection(&self.pool)?;
        let min_date = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_activities::account_id.eq(wf_accounts::id)))
            .filter(wf_accounts::is_archived.eq(false))
            .select(min(wf_activities::activity_date))
            .first::<Option<NaiveDate>>(&mut conn)
            .map_err(StorageError::from)?
            .ok_or(ActivityError::NotFound("No activities found.".to_string()))?;
        Ok(Utc.from_utc_datetime(&min_date.and_hms_opt(0, 0, 0).unwrap_or_default()))
    }

    fn get_first_activity_date(
        &self,
        account_ids: Option<&[String]>,
    ) -> Result<Option<DateTime<Utc>>> {
        let mut conn = get_connection(&self.pool)?;
        let mut query = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_accounts::id.eq(wf_activities::account_id)))
            .filter(wf_accounts::is_archived.eq(false))
            .select(min(wf_activities::activity_date))
            .into_boxed();

        if let Some(ids) = account_ids {
            let parsed_ids = Self::parse_uuid_vec(ids, "account_id")?;
            query = query.filter(wf_activities::account_id.eq_any(parsed_ids));
        }

        Ok(query
            .first::<Option<NaiveDate>>(&mut conn)
            .map_err(StorageError::from)?
            .map(|date| Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap_or_default())))
    }

    fn get_activity_bounds_for_assets(
        &self,
        asset_ids: &[String],
    ) -> Result<HashMap<String, (Option<NaiveDate>, Option<NaiveDate>)>> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let parsed_asset_ids = Self::parse_uuid_vec(asset_ids, "asset_id")?;
        let mut conn = get_connection(&self.pool)?;
        let results = wf_activities::table
            .inner_join(wf_accounts::table.on(wf_activities::account_id.eq(wf_accounts::id)))
            .filter(wf_accounts::is_archived.eq(false))
            .filter(wf_activities::asset_id.eq_any(parsed_asset_ids))
            .group_by(wf_activities::asset_id)
            .select((
                wf_activities::asset_id.assume_not_null(),
                min(wf_activities::activity_date),
                max(wf_activities::activity_date),
            ))
            .load::<(Uuid, Option<NaiveDate>, Option<NaiveDate>)>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results
            .into_iter()
            .map(|(asset_id, first_date, last_date)| (asset_id.to_string(), (first_date, last_date)))
            .collect())
    }

    fn check_existing_duplicates(
        &self,
        idempotency_keys: &[String],
    ) -> Result<HashMap<String, String>> {
        if idempotency_keys.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = get_connection(&self.pool)?;
        let results = wf_activities::table
            .filter(wf_activities::idempotency_key.eq_any(idempotency_keys))
            .select((wf_activities::id, wf_activities::idempotency_key))
            .load::<(Uuid, Option<String>)>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results
            .into_iter()
            .filter_map(|(activity_id, key_opt)| key_opt.map(|key| (key, activity_id.to_string())))
            .collect())
    }

    async fn bulk_upsert(&self, activities_vec: Vec<ActivityUpsert>) -> Result<BulkUpsertResult> {
        use diesel::upsert::excluded;

        if activities_vec.is_empty() {
            return Ok(BulkUpsertResult::default());
        }

        let activity_rows: Vec<ActivityDB> = activities_vec
            .into_iter()
            .map(ActivityDB::from_activity_upsert)
            .collect::<Result<Vec<_>>>()?;

        let mut conn = get_connection(&self.pool)?;
        let activity_ids: Vec<Uuid> = activity_rows.iter().map(|a| a.id).collect();
        let idempotency_keys: Vec<String> = activity_rows
            .iter()
            .filter_map(|a| a.idempotency_key.clone())
            .collect();

        let existing_activities: Vec<(Uuid, Option<String>, bool)> = wf_activities::table
            .filter(
                wf_activities::id
                    .eq_any(&activity_ids)
                    .or(wf_activities::idempotency_key.eq_any(&idempotency_keys)),
            )
            .select((
                wf_activities::id,
                wf_activities::idempotency_key,
                wf_activities::is_user_modified,
            ))
            .load::<(Uuid, Option<String>, bool)>(&mut conn)
            .map_err(StorageError::from)?;

        let mut existing_by_id: HashMap<Uuid, bool> = HashMap::new();
        let mut existing_by_idemp: HashMap<String, (Uuid, bool)> = HashMap::new();
        for (id, idemp_key, is_modified) in existing_activities {
            existing_by_id.insert(id, is_modified);
            if let Some(key) = idemp_key {
                existing_by_idemp.insert(key, (id, is_modified));
            }
        }

        let mut result = BulkUpsertResult::default();
        for mut activity_db in activity_rows {
            let now_update = Utc::now();
            let activity_id = activity_db.id;
            let idempotency_key = activity_db.idempotency_key.clone();

            if let Some(&is_modified) = existing_by_id.get(&activity_id) {
                if is_modified {
                    result.skipped += 1;
                    continue;
                }
            }

            let is_existing = existing_by_id.contains_key(&activity_id);
            if !is_existing {
                if let Some(ref key) = idempotency_key {
                    if let Some((existing_id, is_modified)) = existing_by_idemp.get(key) {
                        if *is_modified {
                            result.skipped += 1;
                            continue;
                        }
                        activity_db.id = *existing_id;
                    }
                }
            }

            let will_update = existing_by_id.contains_key(&activity_db.id)
                || idempotency_key
                    .as_ref()
                    .is_some_and(|key| existing_by_idemp.contains_key(key));

            let count = diesel::insert_into(wf_activities::table)
                .values(&activity_db)
                .on_conflict(wf_activities::id)
                .do_update()
                .set((
                    wf_activities::account_id.eq(excluded(wf_activities::account_id)),
                    wf_activities::asset_id.eq(excluded(wf_activities::asset_id)),
                    wf_activities::activity_type.eq(excluded(wf_activities::activity_type)),
                    wf_activities::subtype.eq(excluded(wf_activities::subtype)),
                    wf_activities::activity_date.eq(excluded(wf_activities::activity_date)),
                    wf_activities::quantity.eq(excluded(wf_activities::quantity)),
                    wf_activities::unit_price.eq(excluded(wf_activities::unit_price)),
                    wf_activities::currency.eq(excluded(wf_activities::currency)),
                    wf_activities::fee.eq(excluded(wf_activities::fee)),
                    wf_activities::amount.eq(excluded(wf_activities::amount)),
                    wf_activities::status.eq(excluded(wf_activities::status)),
                    wf_activities::notes.eq(excluded(wf_activities::notes)),
                    wf_activities::fx_rate.eq(excluded(wf_activities::fx_rate)),
                    wf_activities::metadata.eq(excluded(wf_activities::metadata)),
                    wf_activities::source_system.eq(excluded(wf_activities::source_system)),
                    wf_activities::source_record_id.eq(excluded(wf_activities::source_record_id)),
                    wf_activities::source_group_id.eq(excluded(wf_activities::source_group_id)),
                    wf_activities::needs_review.eq(excluded(wf_activities::needs_review)),
                    wf_activities::idempotency_key.eq(excluded(wf_activities::idempotency_key)),
                    wf_activities::import_run_id.eq(excluded(wf_activities::import_run_id)),
                    wf_activities::updated_at.eq(now_update),
                ))
                .execute(&mut conn)
                .map_err(StorageError::from)?;

            if count > 0 {
                result.upserted += count;
                if will_update {
                    result.updated += count;
                } else {
                    result.created += count;
                }
            }
        }

        Ok(result)
    }

    async fn reassign_asset(&self, old_asset_id: &str, new_asset_id: &str) -> Result<u32> {
        let old_id = Self::parse_uuid(old_asset_id, "old_asset_id")?;
        let new_id = Self::parse_uuid(new_asset_id, "new_asset_id")?;
        let mut conn = get_connection(&self.pool)?;
        let affected_ids = wf_activities::table
            .filter(wf_activities::asset_id.eq(Some(old_id)))
            .select(wf_activities::id)
            .load::<Uuid>(&mut conn)
            .map_err(StorageError::from)?;
        if affected_ids.is_empty() {
            return Ok(0);
        }

        let count = diesel::update(wf_activities::table.filter(wf_activities::asset_id.eq(Some(old_id))))
            .set((
                wf_activities::asset_id.eq(Some(new_id)),
                wf_activities::updated_at.eq(Utc::now()),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;
        Ok(count as u32)
    }

    async fn get_activity_accounts_and_currencies_by_asset_id(
        &self,
        asset_id: &str,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let parsed_asset_id = Self::parse_uuid(asset_id, "asset_id")?;
        let mut conn = get_connection(&self.pool)?;
        let rows: Vec<(Uuid, String)> = wf_activities::table
            .filter(wf_activities::asset_id.eq(Some(parsed_asset_id)))
            .select((wf_activities::account_id, wf_activities::currency))
            .distinct()
            .load(&mut conn)
            .map_err(StorageError::from)?;

        let mut account_ids: HashSet<String> = HashSet::new();
        let mut currencies: HashSet<String> = HashSet::new();
        for (account_id, currency) in rows {
            account_ids.insert(account_id.to_string());
            if !currency.is_empty() {
                currencies.insert(currency);
            }
        }

        Ok((account_ids.into_iter().collect(), currencies.into_iter().collect()))
    }
}