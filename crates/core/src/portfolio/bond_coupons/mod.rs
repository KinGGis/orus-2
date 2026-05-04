//! Bond coupon materialization.
//!
//! For every bond asset whose `quote_mode = INTERNAL_YTM` and that has a
//! computable coupon schedule (via `BondSpec::parse_coupon_schedule()`), this
//! module inspects the activity history per `(account_id, asset_id)` and
//! materializes one `INTEREST` activity per coupon date `<= today` for which
//! the holder had a positive quantity, IF such an activity does not already
//! exist (idempotency key: `BOND_COUPON:{asset_id}:{account_id}:{date}`).
//!
//! The amount per coupon equals
//!   `quantity × face_value × coupon_rate / periods_per_year`
//! and is booked in the bond's `quote_ccy`.
//!
//! This logic runs as part of the portfolio update job, BEFORE snapshots are
//! recalculated, so newly inserted INTEREST activities are picked up by the
//! same recalculation pass and contribute to realized P&L.

use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use log::{debug, info, warn};
use rust_decimal::Decimal;

use crate::activities::{
    ActivityServiceTrait, ActivityType, NewActivity, SymbolInput, ACTIVITY_TYPE_INTEREST,
};
use crate::assets::{AssetServiceTrait, QuoteMode};
use crate::errors::Result;
use crate::portfolio::snapshot::SnapshotRepositoryTrait;

/// Materialize past coupons for all INTERNAL_YTM bonds.
///
/// Returns the number of newly inserted INTEREST activities.
pub async fn materialize_bond_coupons(
    asset_service: Arc<dyn AssetServiceTrait + Send + Sync>,
    activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
    snapshot_repository: Arc<dyn SnapshotRepositoryTrait + Send + Sync>,
    today: NaiveDate,
) -> Result<usize> {
    let assets = asset_service.get_assets()?;
    let mut inserted = 0usize;

    // Pre-fetch all snapshots up to `today` once, then index per (asset, account)
    // by ascending date. This lets us recover ownership for positions that were
    // created via manual entry / broker import without any BUY activities.
    let all_snapshots = snapshot_repository
        .get_all_non_archived_account_snapshots(None, Some(today))?;
    // (asset_id, account_id) -> Vec<(snapshot_date, quantity)> sorted ascending.
    let mut snapshot_qty_index: std::collections::HashMap<
        (String, String),
        Vec<(NaiveDate, Decimal)>,
    > = std::collections::HashMap::new();
    for snap in &all_snapshots {
        // TOTAL aggregates would double-count; skip them.
        if snap.account_id == "TOTAL" {
            continue;
        }
        for (asset_id, position) in &snap.positions {
            snapshot_qty_index
                .entry((asset_id.clone(), snap.account_id.clone()))
                .or_default()
                .push((snap.snapshot_date, position.quantity));
        }
    }
    for v in snapshot_qty_index.values_mut() {
        v.sort_by_key(|(d, _)| *d);
    }

    for asset in assets {
        if asset.quote_mode != QuoteMode::InternalYtm || !asset.is_bond() {
            continue;
        }
        let Some(bond) = asset.bond_spec() else {
            continue;
        };
        let Some(coupon_rate) = bond.coupon_rate else {
            continue;
        };
        let Some(face_value) = bond.face_value else {
            continue;
        };
        if coupon_rate <= Decimal::ZERO || face_value <= Decimal::ZERO {
            continue;
        }
        let schedule = bond.parse_coupon_schedule();
        if schedule.is_empty() {
            continue;
        }
        let Some(periods_per_year) = infer_periods_per_year(&schedule) else {
            warn!(
                "bond_coupons: cannot infer periods/year for asset {} — skipping",
                asset.id
            );
            continue;
        };

        // Per-coupon cash flow per unit held.
        let per_unit = coupon_rate * face_value / Decimal::from(periods_per_year);

        // We need all activities for this asset; the trait does not expose a
        // direct `by_asset_id` getter, so we filter the full list. O(n) but
        // only runs as part of the portfolio job.
        let mut bond_activities: Vec<_> = activity_service
            .get_activities()?
            .into_iter()
            .filter(|a| a.asset_id.as_deref() == Some(asset.id.as_str()) && a.is_posted())
            .collect();
        bond_activities.sort_by_key(|a| a.activity_date);

        // Existing INTEREST activities for this asset, keyed by idempotency_key.
        let mut existing_keys: std::collections::HashSet<String> = bond_activities
            .iter()
            .filter(|a| a.activity_type == ACTIVITY_TYPE_INTEREST)
            .filter_map(|a| a.idempotency_key.clone())
            .collect();

        // Per-account quantity history (date, account, signed delta).
        let mut history: Vec<(NaiveDate, String, Decimal)> = Vec::new();
        for act in &bond_activities {
            let date = act.effective_date();
            let acct = act.account_id.clone();
            let qty = act.qty();
            let delta = match ActivityType::from_str(&act.activity_type) {
                Ok(ActivityType::Buy) | Ok(ActivityType::TransferIn) => qty,
                Ok(ActivityType::Sell) | Ok(ActivityType::TransferOut) => -qty,
                _ => Decimal::ZERO,
            };
            if delta != Decimal::ZERO {
                history.push((date, acct, delta));
            }
        }

        // Helper: qty held on a given date by a given account.
        // Strategy:
        //   1. If the account has any BUY/SELL activity for this bond, use the
        //      activity-derived running sum (canonical accounting view).
        //   2. Otherwise, fall back to snapshot quantity:
        //      - On/after the earliest snapshot containing the asset: use the
        //        latest snapshot ≤ coupon_date.
        //      - Before the earliest snapshot but on/after `effective_start`
        //        (= min(earliest_snapshot_date, first_coupon_date)): use the
        //        earliest snapshot's quantity. This handles the common case
        //        where a user records a manual position today for a bond
        //        whose first coupon was paid a few days ago — they obviously
        //        owned it on the coupon date too.
        let activity_accounts: std::collections::HashSet<String> =
            history.iter().map(|(_, a, _)| a.clone()).collect();
        let qty_on = |account: &str, on: NaiveDate| -> Decimal {
            if activity_accounts.contains(account) {
                return history
                    .iter()
                    .filter(|(d, a, _)| a == account && *d <= on)
                    .map(|(_, _, q)| *q)
                    .sum::<Decimal>();
            }
            let Some(snaps) =
                snapshot_qty_index.get(&(asset.id.clone(), account.to_string()))
            else {
                return Decimal::ZERO;
            };
            // Latest snapshot ≤ on, if any.
            if let Some((_, q)) = snaps.iter().rev().find(|(d, _)| *d <= on) {
                return *q;
            }
            // No snapshot at/before `on`. Allow back-dating to the earliest
            // snapshot's quantity if `on` is within the bond's lifecycle:
            // i.e., on >= min(earliest_snapshot_date, first_coupon_date).
            let (earliest_date, earliest_qty) = snaps.first().copied().unwrap();
            let effective_start = match bond.first_coupon_date {
                Some(fcd) if fcd < earliest_date => fcd,
                _ => earliest_date,
            };
            if on >= effective_start {
                earliest_qty
            } else {
                Decimal::ZERO
            }
        };

        // Distinct accounts holding the bond at any point: union of
        // activity-driven accounts AND snapshot-driven accounts.
        let mut accounts: std::collections::HashSet<String> = activity_accounts.clone();
        for ((aid, acct), _) in &snapshot_qty_index {
            if aid == &asset.id {
                accounts.insert(acct.clone());
            }
        }

        for coupon_date in schedule {
            if coupon_date > today {
                break;
            }
            // Skip the maturity date "synthetic" entry: we only materialize
            // coupon income, not the principal redemption (which happens
            // implicitly when the bond matures and price -> face).
            if Some(coupon_date) == bond.maturity_date {
                continue;
            }

            for account in &accounts {
                let qty = qty_on(account, coupon_date);
                if qty <= Decimal::ZERO {
                    continue;
                }
                let key = format!("BOND_COUPON:{}:{}:{}", asset.id, account, coupon_date);
                if existing_keys.contains(&key) {
                    continue;
                }
                let amount = (per_unit * qty).round_dp(8);
                if amount <= Decimal::ZERO {
                    continue;
                }
                let activity = NewActivity {
                    id: None,
                    account_id: account.clone(),
                    symbol: Some(SymbolInput {
                        id: Some(asset.id.clone()),
                        ..Default::default()
                    }),
                    activity_type: ACTIVITY_TYPE_INTEREST.to_string(),
                    subtype: None,
                    activity_date: coupon_date.format("%Y-%m-%d").to_string(),
                    quantity: None,
                    unit_price: None,
                    currency: asset.quote_ccy.clone(),
                    fee: None,
                    amount: Some(amount),
                    status: None,
                    notes: Some(format!(
                        "Auto-generated coupon ({}): {} × {} @ {:.4}%",
                        coupon_date,
                        qty,
                        face_value,
                        coupon_rate * Decimal::from(100)
                    )),
                    fx_rate: None,
                    metadata: None,
                    needs_review: None,
                    source_system: Some("BOND_ENGINE".to_string()),
                    source_record_id: None,
                    source_group_id: None,
                    idempotency_key: Some(key.clone()),
                };
                match activity_service.create_activity(activity).await {
                    Ok(created) => {
                        debug!(
                            "bond_coupons: inserted INTEREST {} for asset {} account {} date {} amount {}",
                            created.id, asset.id, account, coupon_date, amount
                        );
                        existing_keys.insert(key);
                        inserted += 1;
                    }
                    Err(e) => {
                        warn!(
                            "bond_coupons: failed to insert coupon for asset {} account {} date {}: {}",
                            asset.id, account, coupon_date, e
                        );
                    }
                }
            }
        }
    }

    if inserted > 0 {
        info!("bond_coupons: materialized {} INTEREST activities", inserted);
    }
    Ok(inserted)
}

/// Estimate periods-per-year from the median gap between consecutive coupons.
fn infer_periods_per_year(schedule: &[NaiveDate]) -> Option<u32> {
    if schedule.len() < 2 {
        return None;
    }
    let mut gaps: Vec<i64> = schedule
        .windows(2)
        .map(|w| (w[1] - w[0]).num_days())
        .collect();
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    Some(match median {
        ..=35 => 12,
        36..=100 => 4,
        101..=200 => 2,
        _ => 1,
    })
}
