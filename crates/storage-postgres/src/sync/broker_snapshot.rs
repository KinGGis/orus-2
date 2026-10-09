//! PostgreSQL persistence for broker snapshots.

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use rust_decimal::Decimal;
use serde_json::Value;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::schema::{wf_broker_snapshot_metrics, wf_broker_snapshots};
use wealthfolio_connect::broker_ingest::{
    BrokerSnapshotKind, BrokerSnapshotRepositoryTrait, NewBrokerSnapshot, StoredBrokerSnapshot,
};
use wealthfolio_core::errors::ValidationError;
use wealthfolio_core::{Error, Result};

#[derive(Queryable, Insertable, Selectable, Debug, Clone)]
#[diesel(table_name = wf_broker_snapshots)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct BrokerSnapshotDB {
    id: Uuid,
    account_id: Uuid,
    provider: String,
    kind: String,
    as_of_date: NaiveDate,
    captured_at: DateTime<Utc>,
    payload: Value,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = wf_broker_snapshot_metrics)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct BrokerSnapshotMetricDB {
    id: Uuid,
    snapshot_id: Uuid,
    account_id: Uuid,
    as_of_date: NaiveDate,
    dimension: String,
    category_id: String,
    category_name: Option<String>,
    side: String,
    value: Option<BigDecimal>,
    weight: Option<BigDecimal>,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        Error::Validation(ValidationError::InvalidInput(format!(
            "Invalid {field} UUID: {err}"
        )))
    })
}

fn to_bigdecimal(value: Option<Decimal>) -> Option<BigDecimal> {
    value.and_then(|value| BigDecimal::from_str(&value.to_string()).ok())
}

pub struct BrokerSnapshotRepository {
    pool: Arc<DbPool>,
}

impl BrokerSnapshotRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BrokerSnapshotRepositoryTrait for BrokerSnapshotRepository {
    async fn upsert(&self, snapshot: NewBrokerSnapshot) -> Result<()> {
        let account_id = parse_uuid(&snapshot.account_id, "account_id")?;
        let snapshot_id = Uuid::new_v4();

        let row = BrokerSnapshotDB {
            id: snapshot_id,
            account_id,
            provider: snapshot.provider.clone(),
            kind: snapshot.kind.as_str().to_string(),
            as_of_date: snapshot.as_of_date,
            captured_at: Utc::now(),
            payload: snapshot.payload,
        };

        let metrics: Vec<BrokerSnapshotMetricDB> = snapshot
            .metrics
            .into_iter()
            .map(|metric| BrokerSnapshotMetricDB {
                id: Uuid::new_v4(),
                snapshot_id,
                account_id,
                as_of_date: snapshot.as_of_date,
                dimension: metric.dimension,
                category_id: metric.category_id,
                category_name: metric.category_name,
                side: metric.side.as_str().to_string(),
                value: to_bigdecimal(metric.value),
                weight: to_bigdecimal(metric.weight),
            })
            .collect();

        let mut conn = get_connection(&self.pool)?;

        // Replacing the day rather than updating it in place keeps the metric
        // rows consistent with the payload: a category the broker stopped
        // reporting disappears with the old row through the cascade, instead of
        // lingering next to the new figures.
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::delete(
                wf_broker_snapshots::table
                    .filter(wf_broker_snapshots::account_id.eq(account_id))
                    .filter(wf_broker_snapshots::provider.eq(&snapshot.provider))
                    .filter(wf_broker_snapshots::kind.eq(snapshot.kind.as_str()))
                    .filter(wf_broker_snapshots::as_of_date.eq(snapshot.as_of_date)),
            )
            .execute(conn)?;

            diesel::insert_into(wf_broker_snapshots::table)
                .values(&row)
                .execute(conn)?;

            if !metrics.is_empty() {
                diesel::insert_into(wf_broker_snapshot_metrics::table)
                    .values(&metrics)
                    .execute(conn)?;
            }

            Ok(())
        })
        .map_err(|err| Error::Repository(err.to_string()))?;

        Ok(())
    }

    fn latest(
        &self,
        account_id: &str,
        kind: BrokerSnapshotKind,
    ) -> Result<Option<StoredBrokerSnapshot>> {
        let account_id = parse_uuid(account_id, "account_id")?;
        let mut conn = get_connection(&self.pool)?;

        let row = wf_broker_snapshots::table
            .filter(wf_broker_snapshots::account_id.eq(account_id))
            .filter(wf_broker_snapshots::kind.eq(kind.as_str()))
            .order(wf_broker_snapshots::as_of_date.desc())
            .select(BrokerSnapshotDB::as_select())
            .first::<BrokerSnapshotDB>(&mut conn)
            .optional()
            .map_err(|err| Error::Repository(err.to_string()))?;

        Ok(row.map(|row| StoredBrokerSnapshot {
            as_of_date: row.as_of_date,
            captured_at: row.captured_at,
            payload: row.payload,
        }))
    }
}
