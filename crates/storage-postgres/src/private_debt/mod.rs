//! PostgreSQL persistence for private debt schedules and rate fixings.

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::schema::{wf_debt_schedule_flows, wf_rate_fixings};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::private_debt::{
    DebtFlowSettlement, DebtFlowStatus, DebtFlowType, DebtScheduleFlow,
    DebtScheduleRepositoryTrait, NewDebtScheduleFlow, RateFixing, RateFixingRepositoryTrait,
};
use wealthfolio_core::{Error, Result};

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = wf_debt_schedule_flows)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct DebtScheduleFlowDB {
    id: Uuid,
    account_id: Uuid,
    asset_id: Uuid,
    flow_date: NaiveDate,
    flow_type: String,
    period_start: Option<NaiveDate>,
    expected_amount: BigDecimal,
    currency: String,
    rate_applied: Option<BigDecimal>,
    principal_before: Option<BigDecimal>,
    status: String,
    received_amount: Option<BigDecimal>,
    received_date: Option<NaiveDate>,
    activity_id: Option<Uuid>,
    notes: Option<String>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = wf_debt_schedule_flows)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct NewDebtScheduleFlowDB {
    id: Uuid,
    account_id: Uuid,
    asset_id: Uuid,
    flow_date: NaiveDate,
    flow_type: String,
    period_start: Option<NaiveDate>,
    expected_amount: BigDecimal,
    currency: String,
    rate_applied: Option<BigDecimal>,
    principal_before: Option<BigDecimal>,
}

#[derive(AsChangeset, Debug, Clone)]
#[diesel(table_name = wf_debt_schedule_flows, treat_none_as_null = true)]
struct DebtFlowSettlementDB {
    status: String,
    received_amount: Option<BigDecimal>,
    received_date: Option<NaiveDate>,
    activity_id: Option<Uuid>,
    notes: Option<String>,
    updated_at: DateTime<Utc>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = wf_rate_fixings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct RateFixingDB {
    id: Uuid,
    index_code: String,
    fixing_date: NaiveDate,
    rate: BigDecimal,
    source: String,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn to_bigdecimal(value: Decimal) -> BigDecimal {
    BigDecimal::from_str(&value.to_string()).unwrap_or_default()
}

fn to_decimal(value: &BigDecimal) -> Decimal {
    Decimal::from_str(&value.normalized().to_string()).unwrap_or_default()
}

fn repo_err(err: diesel::result::Error) -> Error {
    Error::Repository(err.to_string())
}

impl TryFrom<DebtScheduleFlowDB> for DebtScheduleFlow {
    type Error = Error;

    fn try_from(row: DebtScheduleFlowDB) -> Result<Self> {
        Ok(Self {
            id: row.id.to_string(),
            account_id: row.account_id.to_string(),
            asset_id: row.asset_id.to_string(),
            flow_date: row.flow_date,
            flow_type: DebtFlowType::parse(&row.flow_type)?,
            period_start: row.period_start,
            expected_amount: to_decimal(&row.expected_amount),
            currency: row.currency,
            rate_applied: row.rate_applied.as_ref().map(to_decimal),
            principal_before: row.principal_before.as_ref().map(to_decimal),
            status: DebtFlowStatus::parse(&row.status)?,
            received_amount: row.received_amount.as_ref().map(to_decimal),
            received_date: row.received_date,
            activity_id: row.activity_id.map(|id| id.to_string()),
            notes: row.notes,
        })
    }
}

impl From<RateFixingDB> for RateFixing {
    fn from(row: RateFixingDB) -> Self {
        Self {
            index_code: row.index_code,
            fixing_date: row.fixing_date,
            rate: to_decimal(&row.rate),
            source: row.source,
        }
    }
}

pub struct DebtScheduleRepository {
    pool: Arc<DbPool>,
}

impl DebtScheduleRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DebtScheduleRepositoryTrait for DebtScheduleRepository {
    fn list_flows(&self, account_id: &str, asset_id: &str) -> Result<Vec<DebtScheduleFlow>> {
        let account_id = parse_uuid(account_id, "account_id")?;
        let asset_id = parse_uuid(asset_id, "asset_id")?;
        let mut conn = get_connection(&self.pool)?;

        wf_debt_schedule_flows::table
            .filter(wf_debt_schedule_flows::account_id.eq(account_id))
            .filter(wf_debt_schedule_flows::asset_id.eq(asset_id))
            .order((
                wf_debt_schedule_flows::flow_date.asc(),
                wf_debt_schedule_flows::flow_type.asc(),
            ))
            .select(DebtScheduleFlowDB::as_select())
            .load::<DebtScheduleFlowDB>(&mut conn)
            .map_err(repo_err)?
            .into_iter()
            .map(DebtScheduleFlow::try_from)
            .collect()
    }

    fn list_flows_due(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<DebtScheduleFlow>> {
        let mut conn = get_connection(&self.pool)?;

        wf_debt_schedule_flows::table
            .filter(wf_debt_schedule_flows::flow_date.between(from, to))
            .order((
                wf_debt_schedule_flows::flow_date.asc(),
                wf_debt_schedule_flows::asset_id.asc(),
            ))
            .select(DebtScheduleFlowDB::as_select())
            .load::<DebtScheduleFlowDB>(&mut conn)
            .map_err(repo_err)?
            .into_iter()
            .map(DebtScheduleFlow::try_from)
            .collect()
    }

    async fn replace_expected_flows(
        &self,
        account_id: &str,
        asset_id: &str,
        flows: Vec<NewDebtScheduleFlow>,
    ) -> Result<usize> {
        let account_id = parse_uuid(account_id, "account_id")?;
        let asset_id = parse_uuid(asset_id, "asset_id")?;

        let rows: Vec<NewDebtScheduleFlowDB> = flows
            .into_iter()
            .map(|flow| NewDebtScheduleFlowDB {
                id: Uuid::new_v4(),
                account_id,
                asset_id,
                flow_date: flow.flow_date,
                flow_type: flow.flow_type.as_str().to_string(),
                period_start: flow.period_start,
                expected_amount: to_bigdecimal(flow.expected_amount),
                currency: flow.currency,
                rate_applied: flow.rate_applied.map(to_bigdecimal),
                principal_before: flow.principal_before.map(to_bigdecimal),
            })
            .collect();

        let mut conn = get_connection(&self.pool)?;

        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::delete(
                wf_debt_schedule_flows::table
                    .filter(wf_debt_schedule_flows::account_id.eq(account_id))
                    .filter(wf_debt_schedule_flows::asset_id.eq(asset_id))
                    .filter(wf_debt_schedule_flows::status.eq(DebtFlowStatus::Expected.as_str()))
                    .filter(wf_debt_schedule_flows::activity_id.is_null()),
            )
            .execute(conn)?;

            if rows.is_empty() {
                return Ok(0);
            }

            // Whatever survived the delete was reconciled: it wins over the
            // regenerated flow for the same date and type.
            diesel::insert_into(wf_debt_schedule_flows::table)
                .values(&rows)
                .on_conflict((
                    wf_debt_schedule_flows::account_id,
                    wf_debt_schedule_flows::asset_id,
                    wf_debt_schedule_flows::flow_date,
                    wf_debt_schedule_flows::flow_type,
                ))
                .do_nothing()
                .execute(conn)
        })
        .map_err(repo_err)
    }

    async fn settle_flow(
        &self,
        flow_id: &str,
        settlement: DebtFlowSettlement,
    ) -> Result<DebtScheduleFlow> {
        let flow_id = parse_uuid(flow_id, "flow_id")?;
        let activity_id = settlement
            .activity_id
            .as_deref()
            .map(|id| parse_uuid(id, "activity_id"))
            .transpose()?;

        let changes = DebtFlowSettlementDB {
            status: settlement.status.as_str().to_string(),
            received_amount: settlement.received_amount.map(to_bigdecimal),
            received_date: settlement.received_date,
            activity_id,
            notes: settlement.notes,
            updated_at: Utc::now(),
        };

        let mut conn = get_connection(&self.pool)?;

        diesel::update(wf_debt_schedule_flows::table.find(flow_id))
            .set(&changes)
            .returning(DebtScheduleFlowDB::as_returning())
            .get_result::<DebtScheduleFlowDB>(&mut conn)
            .map_err(repo_err)?
            .try_into()
    }
}

pub struct RateFixingRepository {
    pool: Arc<DbPool>,
}

impl RateFixingRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RateFixingRepositoryTrait for RateFixingRepository {
    fn fixing_on(&self, index_code: &str, date: NaiveDate) -> Result<Option<RateFixing>> {
        let mut conn = get_connection(&self.pool)?;

        let row = wf_rate_fixings::table
            .filter(wf_rate_fixings::index_code.eq(index_code))
            .filter(wf_rate_fixings::fixing_date.le(date))
            .order(wf_rate_fixings::fixing_date.desc())
            .select(RateFixingDB::as_select())
            .first::<RateFixingDB>(&mut conn)
            .optional()
            .map_err(repo_err)?;

        Ok(row.map(RateFixing::from))
    }

    async fn upsert_fixings(&self, fixings: Vec<RateFixing>) -> Result<usize> {
        if fixings.is_empty() {
            return Ok(0);
        }

        let rows: Vec<RateFixingDB> = fixings
            .into_iter()
            .map(|fixing| RateFixingDB {
                id: Uuid::new_v4(),
                index_code: fixing.index_code,
                fixing_date: fixing.fixing_date,
                rate: to_bigdecimal(fixing.rate),
                source: fixing.source,
            })
            .collect();

        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(wf_rate_fixings::table)
            .values(&rows)
            .on_conflict((wf_rate_fixings::index_code, wf_rate_fixings::fixing_date))
            .do_update()
            .set((
                wf_rate_fixings::rate.eq(excluded(wf_rate_fixings::rate)),
                wf_rate_fixings::source.eq(excluded(wf_rate_fixings::source)),
            ))
            .execute(&mut conn)
            .map_err(repo_err)
    }
}

/// Runs against a real database: set `TEST_DATABASE_URL` to a disposable
/// Postgres (migrations are applied to it). Skipped otherwise.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_pool, run_migrations};
    use diesel::sql_query;
    use diesel::sql_types::Uuid as SqlUuid;
    use rust_decimal_macros::dec;

    fn pool() -> Option<Arc<DbPool>> {
        static MIGRATED: std::sync::Once = std::sync::Once::new();
        let url = std::env::var("TEST_DATABASE_URL").ok()?;
        MIGRATED.call_once(|| run_migrations(&url).expect("migrations"));
        Some(create_pool(&url).expect("pool"))
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn seed_loan(pool: &DbPool) -> (String, String) {
        let account_id = Uuid::new_v4();
        let asset_id = Uuid::new_v4();
        let mut conn = get_connection(pool).unwrap();
        sql_query("INSERT INTO wf_accounts (id, name, account_type) VALUES ($1, 'Debt', 'SECURITIES')")
            .bind::<SqlUuid, _>(account_id)
            .execute(&mut conn)
            .unwrap();
        sql_query("INSERT INTO wf_assets (id, kind, instrument_type) VALUES ($1, 'INVESTMENT', 'BOND')")
            .bind::<SqlUuid, _>(asset_id)
            .execute(&mut conn)
            .unwrap();
        (account_id.to_string(), asset_id.to_string())
    }

    fn interest(day: &str, amount: Decimal) -> NewDebtScheduleFlow {
        NewDebtScheduleFlow {
            flow_date: date(day),
            flow_type: DebtFlowType::InterestCash,
            period_start: None,
            expected_amount: amount,
            currency: "EUR".to_string(),
            rate_applied: Some(dec!(0.146)),
            principal_before: Some(dec!(150)),
        }
    }

    #[tokio::test]
    async fn regenerating_a_schedule_keeps_settled_flows() {
        let Some(pool) = pool() else { return };
        let (account, asset) = seed_loan(&pool);
        let repo = DebtScheduleRepository::new(pool);

        let inserted = repo
            .replace_expected_flows(
                &account,
                &asset,
                vec![
                    interest("2026-11-17", dec!(1.83)),
                    interest("2026-12-17", dec!(1.83)),
                ],
            )
            .await
            .unwrap();
        assert_eq!(inserted, 2);

        let november = repo.list_flows(&account, &asset).unwrap().remove(0);
        let settled = repo
            .settle_flow(
                &november.id,
                DebtFlowSettlement {
                    status: DebtFlowStatus::Received,
                    received_amount: Some(dec!(1.80)),
                    received_date: Some(date("2026-11-18")),
                    activity_id: None,
                    notes: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(settled.status, DebtFlowStatus::Received);
        assert_eq!(settled.received_amount, Some(dec!(1.8)));

        // New terms: every amount changes, and a January flow appears.
        let inserted = repo
            .replace_expected_flows(
                &account,
                &asset,
                vec![
                    interest("2026-11-17", dec!(2)),
                    interest("2026-12-17", dec!(2)),
                    interest("2027-01-17", dec!(2)),
                ],
            )
            .await
            .unwrap();
        assert_eq!(inserted, 2);

        let flows = repo.list_flows(&account, &asset).unwrap();
        let amounts: Vec<_> = flows
            .iter()
            .map(|f| (f.flow_date, f.status, f.expected_amount))
            .collect();
        assert_eq!(
            amounts,
            vec![
                (date("2026-11-17"), DebtFlowStatus::Received, dec!(1.83)),
                (date("2026-12-17"), DebtFlowStatus::Expected, dec!(2)),
                (date("2027-01-17"), DebtFlowStatus::Expected, dec!(2)),
            ]
        );
        assert_eq!(
            repo.list_flows_due(date("2026-12-01"), date("2026-12-31"))
                .unwrap()
                .into_iter()
                .filter(|f| f.asset_id == asset)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn fixings_resolve_to_the_latest_on_or_before_a_date() {
        let Some(pool) = pool() else { return };
        let repo = RateFixingRepository::new(pool);
        let index = format!("TEST_{}", Uuid::new_v4().simple());
        let fixing = |day: &str, rate: Decimal| RateFixing {
            index_code: index.clone(),
            fixing_date: date(day),
            rate,
            source: "MANUAL".to_string(),
        };

        repo.upsert_fixings(vec![fixing("2026-10-01", dec!(0.02)), fixing("2026-11-01", dec!(0.021))])
            .await
            .unwrap();
        repo.upsert_fixings(vec![fixing("2026-11-01", dec!(0.0215))])
            .await
            .unwrap();

        assert_eq!(repo.fixing_on(&index, date("2026-09-30")).unwrap(), None);
        assert_eq!(
            repo.fixing_on(&index, date("2026-10-15")).unwrap().unwrap().rate,
            dec!(0.02)
        );
        assert_eq!(
            repo.fixing_on(&index, date("2026-11-19")).unwrap().unwrap().rate,
            dec!(0.0215)
        );
    }
}
