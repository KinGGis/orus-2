//! Turn raw IBKR payloads into historisable snapshots.
//!
//! Each builder keeps the payload verbatim — that is what an audit needs — and
//! flattens the figures it carries into one metric row per category so the same
//! history is queryable without parsing JSON.

use chrono::NaiveDate;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde_json::Value;
use wealthfolio_core::errors::{Error, Result};

use crate::broker_ingest::{
    BrokerSnapshotKind, BrokerSnapshotMetric, BrokerSnapshotSide, NewBrokerSnapshot,
};

use super::allocation::{parse_allocations, AllocationBucket, AllocationResponse};
use super::client::IBKR_SOURCE_SYSTEM;
use super::models::{IbkrAccountSummary, IbkrPerformanceResponse};

fn decimal(value: Option<f64>) -> Option<Decimal> {
    value.and_then(Decimal::from_f64).map(|v| v.round_dp(6))
}

fn snapshot(
    account_id: &str,
    kind: BrokerSnapshotKind,
    as_of_date: NaiveDate,
    payload: Value,
    metrics: Vec<BrokerSnapshotMetric>,
) -> NewBrokerSnapshot {
    NewBrokerSnapshot {
        account_id: account_id.to_string(),
        provider: IBKR_SOURCE_SYSTEM.to_string(),
        kind,
        as_of_date,
        payload,
        metrics,
    }
}

fn bucket_metrics(
    dimension: &str,
    side: BrokerSnapshotSide,
    bucket: Option<&AllocationBucket>,
) -> Vec<BrokerSnapshotMetric> {
    let Some(bucket) = bucket else {
        return Vec::new();
    };

    let total = bucket
        .total
        .as_ref()
        .and_then(|total| decimal(total.nav))
        .filter(|total| !total.is_zero());

    bucket
        .items
        .iter()
        .filter_map(|item| {
            let id = item.id.as_ref().filter(|id| !id.is_empty())?;
            let value = decimal(item.nav);

            Some(BrokerSnapshotMetric {
                dimension: dimension.to_string(),
                category_id: id.clone(),
                category_name: item.name.clone().filter(|name| !name.is_empty()),
                side,
                value,
                weight: match (value, total) {
                    (Some(value), Some(total)) => Some((value / total).round_dp(6)),
                    _ => None,
                },
            })
        })
        .collect()
}

fn allocation_metrics(response: &AllocationResponse) -> Vec<BrokerSnapshotMetric> {
    let mut dimensions: Vec<&String> = response.allocations.keys().collect();
    // A HashMap iterates in an arbitrary order; sorting keeps a re-sync of the
    // same payload producing the same rows, which makes diffing two captures
    // meaningful.
    dimensions.sort();

    dimensions
        .into_iter()
        .flat_map(|key| {
            let dimension = &response.allocations[key];
            let mut metrics = bucket_metrics(
                key,
                BrokerSnapshotSide::Long,
                dimension.long_positions.as_ref(),
            );
            metrics.extend(bucket_metrics(
                key,
                BrokerSnapshotSide::Short,
                dimension.short_positions.as_ref(),
            ));
            metrics
        })
        .collect()
}

pub fn allocation_snapshot(
    account_id: &str,
    as_of_date: NaiveDate,
    payload: Value,
) -> Result<NewBrokerSnapshot> {
    let response = parse_allocations(payload.clone())?;

    Ok(snapshot(
        account_id,
        BrokerSnapshotKind::Allocation,
        as_of_date,
        payload,
        allocation_metrics(&response),
    ))
}

/// Flatten the account metrics IBKR reports.
///
/// Only two of these were ever read; the rest are what tells an audit how much
/// of a past valuation rested on leverage, so all of them are recorded.
fn account_summary_metrics(summary: &IbkrAccountSummary) -> Vec<BrokerSnapshotMetric> {
    [
        ("NET_LIQUIDATION", summary.net_liquidation),
        ("EQUITY_WITH_LOAN_VALUE", summary.equity_with_loan_value),
        ("BUYING_POWER", summary.buying_power),
        ("GROSS_POSITION_VALUE", summary.gross_position_value),
        ("TOTAL_CASH_VALUE", summary.total_cash_value),
        ("AVAILABLE_FUNDS", summary.available_funds),
        ("EXCESS_LIQUIDITY", summary.excess_liquidity),
    ]
    .into_iter()
    .filter_map(|(id, value)| {
        Some(BrokerSnapshotMetric {
            dimension: "ACCOUNT_SUMMARY".to_string(),
            category_id: id.to_string(),
            category_name: summary.currency.clone(),
            side: BrokerSnapshotSide::Net,
            value: Some(decimal(value)?),
            weight: None,
        })
    })
    .collect()
}

pub fn account_summary_snapshot(
    account_id: &str,
    as_of_date: NaiveDate,
    payload: Value,
) -> Result<NewBrokerSnapshot> {
    let summary: IbkrAccountSummary = serde_json::from_value(payload.clone()).map_err(|err| {
        Error::Unexpected(format!("Unexpected IBKR account summary shape: {err}"))
    })?;

    Ok(snapshot(
        account_id,
        BrokerSnapshotKind::AccountSummary,
        as_of_date,
        payload,
        account_summary_metrics(&summary),
    ))
}

/// Keep the broker's own return for each window.
///
/// `cps` is cumulative from the start of the window, so its last point is the
/// return over that window. Recording it means the track record can later be
/// checked against what the custodian reported, instead of only against what we
/// recompute from quotes.
fn performance_metrics(response: &IbkrPerformanceResponse) -> Vec<BrokerSnapshotMetric> {
    let Some(account) = response
        .accounts
        .as_ref()
        .and_then(|accounts| accounts.account.as_ref())
    else {
        return Vec::new();
    };

    let Some(periods) = account.periods.as_ref() else {
        return Vec::new();
    };

    let mut labels: Vec<&String> = periods.keys().collect();
    labels.sort();

    labels
        .into_iter()
        .filter_map(|label| {
            let period = &periods[label];
            let value = decimal(period.cps.as_ref().and_then(|cps| cps.last()).copied())?;

            Some(BrokerSnapshotMetric {
                dimension: "PERFORMANCE".to_string(),
                category_id: label.to_uppercase(),
                category_name: account.base_currency.clone(),
                side: BrokerSnapshotSide::Net,
                value: Some(value),
                weight: None,
            })
        })
        .collect()
}

pub fn performance_snapshot(
    account_id: &str,
    as_of_date: NaiveDate,
    payload: Value,
) -> Result<NewBrokerSnapshot> {
    let response: IbkrPerformanceResponse = serde_json::from_value(payload.clone())
        .map_err(|err| Error::Unexpected(format!("Unexpected IBKR performance shape: {err}")))?;

    Ok(snapshot(
        account_id,
        BrokerSnapshotKind::Performance,
        as_of_date,
        payload,
        performance_metrics(&response),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use serde_json::json;

    fn metric<'a>(
        metrics: &'a [BrokerSnapshotMetric],
        dimension: &str,
        category_id: &str,
        side: BrokerSnapshotSide,
    ) -> &'a BrokerSnapshotMetric {
        metrics
            .iter()
            .find(|metric| {
                metric.dimension == dimension
                    && metric.category_id == category_id
                    && metric.side == side
            })
            .expect("metric should be present")
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 9).unwrap()
    }

    fn allocation_payload() -> Value {
        json!({
            "allocations": {
                "ASSET_CLASS": {
                    "long_positions": {
                        "total": { "nav": 100.0 },
                        "items": [
                            { "id": "EQ", "name": "Equities", "nav": 75.0 },
                            { "id": "CA", "name": "Cash", "nav": 25.0 }
                        ]
                    },
                    "short_positions": {
                        "total": { "nav": -20.0 },
                        "items": [{ "id": "CA", "name": "Cash", "nav": -20.0 }]
                    }
                }
            }
        })
    }

    #[test]
    fn allocation_snapshot_keeps_the_payload_verbatim() {
        let payload = allocation_payload();
        let snapshot = allocation_snapshot("acc-1", date(), payload.clone()).unwrap();

        assert_eq!(snapshot.payload, payload);
        assert_eq!(snapshot.kind, BrokerSnapshotKind::Allocation);
        assert_eq!(snapshot.provider, IBKR_SOURCE_SYSTEM);
    }

    #[test]
    fn allocation_metrics_keep_long_and_short_legs_apart() {
        let snapshot = allocation_snapshot("acc-1", date(), allocation_payload()).unwrap();

        // Netting the two cash legs would report a 5.0 position that the broker
        // never held on either side of the book.
        let long_cash = metric(
            &snapshot.metrics,
            "ASSET_CLASS",
            "CA",
            BrokerSnapshotSide::Long,
        );
        let short_cash = metric(
            &snapshot.metrics,
            "ASSET_CLASS",
            "CA",
            BrokerSnapshotSide::Short,
        );

        assert_eq!(long_cash.value, Some(dec!(25)));
        assert_eq!(short_cash.value, Some(dec!(-20)));
    }

    #[test]
    fn allocation_weights_are_taken_against_their_own_bucket() {
        let snapshot = allocation_snapshot("acc-1", date(), allocation_payload()).unwrap();

        let equities = metric(
            &snapshot.metrics,
            "ASSET_CLASS",
            "EQ",
            BrokerSnapshotSide::Long,
        );

        assert_eq!(equities.weight, Some(dec!(0.75)));
        assert_eq!(equities.category_name.as_deref(), Some("Equities"));
    }

    #[test]
    fn account_summary_records_every_reported_metric() {
        let payload = json!({
            "currency": "USD",
            "net_liquidation": 4131.3,
            "buying_power": 16525.2,
            "excess_liquidity": 3800.0
        });

        let snapshot = account_summary_snapshot("acc-1", date(), payload).unwrap();

        // Absent keys must not become zeroes: a missing figure and a figure of
        // zero mean different things to an audit.
        assert_eq!(snapshot.metrics.len(), 3);
        assert_eq!(
            metric(
                &snapshot.metrics,
                "ACCOUNT_SUMMARY",
                "BUYING_POWER",
                BrokerSnapshotSide::Net
            )
            .value,
            Some(dec!(16525.2))
        );
    }

    #[test]
    fn performance_keeps_the_last_cumulative_point_of_each_window() {
        let payload = json!({
            "accounts": {
                "account": {
                    "base_currency": "USD",
                    "periods": {
                        "YTD": { "dates": ["20260101", "20261009"], "cps": [0.0, 0.1234] },
                        "1D": { "dates": ["20261009"], "cps": [0.004] }
                    }
                }
            }
        });

        let snapshot = performance_snapshot("acc-1", date(), payload).unwrap();

        assert_eq!(
            metric(
                &snapshot.metrics,
                "PERFORMANCE",
                "YTD",
                BrokerSnapshotSide::Net
            )
            .value,
            Some(dec!(0.1234))
        );
        assert_eq!(snapshot.metrics.len(), 2);
    }

    #[test]
    fn performance_tolerates_a_payload_without_periods() {
        let payload = json!({ "accounts": { "account": { "base_currency": "USD" } } });
        let snapshot = performance_snapshot("acc-1", date(), payload).unwrap();

        assert!(snapshot.metrics.is_empty());
    }
}
