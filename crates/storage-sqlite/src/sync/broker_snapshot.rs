//! SQLite persistence for broker snapshots.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use diesel::r2d2::{self, Pool};
use diesel::sqlite::SqliteConnection;
use std::str::FromStr;
use std::sync::Arc;

use wealthfolio_connect::broker_ingest::{
    BrokerSnapshotKind, BrokerSnapshotRepositoryTrait, NewBrokerSnapshot, StoredBrokerSnapshot,
};
use wealthfolio_core::errors::Result;

use crate::db::{get_connection, WriteHandle};
use crate::errors::StorageError;
use crate::schema::{broker_snapshot_metrics, broker_snapshots};

const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Queryable, Insertable, Selectable, Debug, Clone)]
#[diesel(table_name = broker_snapshots)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct BrokerSnapshotDB {
    id: String,
    account_id: String,
    provider: String,
    kind: String,
    as_of_date: String,
    captured_at: String,
    payload: String,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = broker_snapshot_metrics)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct BrokerSnapshotMetricDB {
    id: String,
    snapshot_id: String,
    account_id: String,
    as_of_date: String,
    dimension: String,
    category_id: String,
    category_name: Option<String>,
    side: String,
    value: Option<String>,
    weight: Option<String>,
}

pub struct BrokerSnapshotRepository {
    pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>,
    writer: WriteHandle,
}

impl BrokerSnapshotRepository {
    pub fn new(
        pool: Arc<Pool<r2d2::ConnectionManager<SqliteConnection>>>,
        writer: WriteHandle,
    ) -> Self {
        Self { pool, writer }
    }
}

#[async_trait]
impl BrokerSnapshotRepositoryTrait for BrokerSnapshotRepository {
    async fn upsert(&self, snapshot: NewBrokerSnapshot) -> Result<()> {
        let snapshot_id = uuid::Uuid::new_v4().to_string();
        let as_of_date = snapshot.as_of_date.format(DATE_FORMAT).to_string();
        let payload = serde_json::to_string(&snapshot.payload)?;

        let row = BrokerSnapshotDB {
            id: snapshot_id.clone(),
            account_id: snapshot.account_id.clone(),
            provider: snapshot.provider.clone(),
            kind: snapshot.kind.as_str().to_string(),
            as_of_date: as_of_date.clone(),
            captured_at: Utc::now().to_rfc3339(),
            payload,
        };

        let metrics: Vec<BrokerSnapshotMetricDB> = snapshot
            .metrics
            .into_iter()
            .map(|metric| BrokerSnapshotMetricDB {
                id: uuid::Uuid::new_v4().to_string(),
                snapshot_id: snapshot_id.clone(),
                account_id: snapshot.account_id.clone(),
                as_of_date: as_of_date.clone(),
                dimension: metric.dimension,
                category_id: metric.category_id,
                category_name: metric.category_name,
                side: metric.side.as_str().to_string(),
                value: metric.value.map(|value| value.to_string()),
                weight: metric.weight.map(|weight| weight.to_string()),
            })
            .collect();

        let account_id = snapshot.account_id;
        let provider = snapshot.provider;
        let kind = snapshot.kind.as_str().to_string();

        // Replacing the day rather than updating it in place keeps the metric
        // rows consistent with the payload: a category the broker stopped
        // reporting disappears with the old row through the cascade, instead of
        // lingering next to the new figures.
        self.writer
            .exec(move |conn| {
                conn.transaction::<_, StorageError, _>(|conn| {
                    diesel::delete(
                        broker_snapshots::table
                            .filter(broker_snapshots::account_id.eq(&account_id))
                            .filter(broker_snapshots::provider.eq(&provider))
                            .filter(broker_snapshots::kind.eq(&kind))
                            .filter(broker_snapshots::as_of_date.eq(&as_of_date)),
                    )
                    .execute(conn)?;

                    diesel::insert_into(broker_snapshots::table)
                        .values(&row)
                        .execute(conn)?;

                    if !metrics.is_empty() {
                        diesel::insert_into(broker_snapshot_metrics::table)
                            .values(&metrics)
                            .execute(conn)?;
                    }

                    Ok(())
                })
                .map_err(Into::into)
            })
            .await
    }

    fn latest(
        &self,
        account_id: &str,
        kind: BrokerSnapshotKind,
    ) -> Result<Option<StoredBrokerSnapshot>> {
        let mut conn = get_connection(&self.pool)?;

        let row = broker_snapshots::table
            .filter(broker_snapshots::account_id.eq(account_id))
            .filter(broker_snapshots::kind.eq(kind.as_str()))
            .order(broker_snapshots::as_of_date.desc())
            .select(BrokerSnapshotDB::as_select())
            .first::<BrokerSnapshotDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(StoredBrokerSnapshot {
            as_of_date: NaiveDate::parse_from_str(&row.as_of_date, DATE_FORMAT)
                .unwrap_or_else(|_| Utc::now().date_naive()),
            captured_at: DateTime::parse_from_rfc3339(&row.captured_at)
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            payload: serde_json::Value::from_str(&row.payload)?,
        }))
    }
}
