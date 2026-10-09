//! Historised capture of what a broker reported at a point in time.
//!
//! Positions, trades, quotes and valuations are already persisted by the sync.
//! The figures a broker *derives* — its own allocation breakdown, its account
//! metrics, its period returns — were read live and thrown away, which left two
//! problems: a page could not be rendered without the broker being reachable,
//! and there was no record of what the custodian actually reported on a given
//! day. Track-record audits need exactly that record.
//!
//! Each capture is stored twice, on purpose:
//!
//! - the **raw payload**, verbatim, so an audit can prove what the broker said
//!   and so figures can be re-derived if our interpretation of them changes;
//! - a **flattened metric row per category**, so the same history can be
//!   queried in SQL without digging through JSON.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wealthfolio_core::Result;

/// Which broker response a snapshot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrokerSnapshotKind {
    /// Allocation breakdown by asset class, region, country and sector.
    Allocation,
    /// Account-level metrics such as buying power or excess liquidity.
    AccountSummary,
    /// Returns per trailing period.
    Performance,
}

impl BrokerSnapshotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allocation => "ALLOCATION",
            Self::AccountSummary => "ACCOUNT_SUMMARY",
            Self::Performance => "PERFORMANCE",
        }
    }
}

/// Which side of the book a metric describes.
///
/// Brokers report long and short legs separately, and the net figure is not
/// always their sum once leverage is involved, so the side is recorded rather
/// than collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrokerSnapshotSide {
    Long,
    Short,
    Net,
}

impl BrokerSnapshotSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Long => "LONG",
            Self::Short => "SHORT",
            Self::Net => "NET",
        }
    }
}

/// One queryable figure out of a snapshot.
///
/// `dimension` names the breakdown (`ASSET_CLASS`, `REGION`, `SECTOR`,
/// `ACCOUNT_SUMMARY`, `PERFORMANCE`) and `category_id` the line within it
/// (`EQ`, `EUROPE`, `BUYING_POWER`, `YTD`). Keeping one shape for all of them
/// means a single table answers "how did this figure move over time" whatever
/// the figure is.
#[derive(Debug, Clone, PartialEq)]
pub struct BrokerSnapshotMetric {
    pub dimension: String,
    pub category_id: String,
    pub category_name: Option<String>,
    pub side: BrokerSnapshotSide,
    /// Absolute figure, in the account's base currency where it is a money
    /// amount, or a percentage where it is a return.
    pub value: Option<Decimal>,
    /// Share of its dimension, where the broker reports one.
    pub weight: Option<Decimal>,
}

/// A capture to persist.
#[derive(Debug, Clone)]
pub struct NewBrokerSnapshot {
    pub account_id: String,
    pub provider: String,
    pub kind: BrokerSnapshotKind,
    pub as_of_date: NaiveDate,
    pub payload: Value,
    pub metrics: Vec<BrokerSnapshotMetric>,
}

/// A capture read back, with the raw payload the broker returned.
#[derive(Debug, Clone)]
pub struct StoredBrokerSnapshot {
    pub as_of_date: NaiveDate,
    pub captured_at: DateTime<Utc>,
    pub payload: Value,
}

#[async_trait]
pub trait BrokerSnapshotRepositoryTrait: Send + Sync {
    /// Records a capture, replacing any already held for the same account,
    /// provider, kind and day.
    ///
    /// Syncing twice in one day must not create two conflicting versions of
    /// that day, so the latest capture wins. Earlier days are never touched.
    async fn upsert(&self, snapshot: NewBrokerSnapshot) -> Result<()>;

    /// The most recent capture of a kind for an account, if any.
    fn latest(
        &self,
        account_id: &str,
        kind: BrokerSnapshotKind,
    ) -> Result<Option<StoredBrokerSnapshot>>;
}
