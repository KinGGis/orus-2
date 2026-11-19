//! Keeps every private loan's schedule up to date and settled against the
//! book, as part of the portfolio update job.
//!
//! A private loan is a bond asset whose `valuation_method` is set. Per account
//! holding it:
//! 1. the expected schedule is regenerated from its terms (principal =
//!    quantity bought × face value, accrual from `issue_date` or first purchase);
//! 2. each flow falling due is reconciled with a real activity of the same kind
//!    (INTEREST for interest, SELL for principal) dated within
//!    [`MATCH_WINDOW_DAYS`] of it;
//! 3. a due flow with no real counterpart gets a generated activity, which a
//!    real one (e.g. imported later from the platform statement) replaces as
//!    soon as it is recorded. PIK interest is booked as INTEREST reinvested
//!    through a BUY of the same amount.

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use chrono::NaiveDate;
use log::{info, warn};
use rust_decimal::Decimal;

use super::private_debt_model::{
    DebtFlowSettlement, DebtFlowStatus, DebtFlowType, DebtScheduleFlow,
};
use super::private_debt_traits::{DebtScheduleRepositoryTrait, RateFixingRepositoryTrait};
use super::schedule_engine::build_schedule;
use crate::activities::{
    Activity, ActivityServiceTrait, ActivityType, NewActivity, SymbolInput, ACTIVITY_TYPE_BUY,
    ACTIVITY_TYPE_FEE, ACTIVITY_TYPE_INTEREST, ACTIVITY_TYPE_SELL,
};
use crate::assets::{Asset, AssetServiceTrait, BondSpec};
use crate::errors::Result;

/// Prefix of the idempotency key of activities generated from a schedule.
pub const GENERATED_KEY_PREFIX: &str = "DEBT_FLOW:";
const SOURCE_SYSTEM: &str = "DEBT_ENGINE";
const MATCH_WINDOW_DAYS: i64 = 10;
const AUTO_NOTE: &str = "AUTO";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DebtSyncSummary {
    pub loans: usize,
    pub activities_created: usize,
    pub flows_settled: usize,
    pub failures: usize,
}

pub async fn sync_private_debt(
    asset_service: Arc<dyn AssetServiceTrait + Send + Sync>,
    activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
    schedules: Arc<dyn DebtScheduleRepositoryTrait>,
    fixings: Arc<dyn RateFixingRepositoryTrait>,
    today: NaiveDate,
) -> Result<DebtSyncSummary> {
    let loans: Vec<(Asset, BondSpec)> = asset_service
        .get_assets()?
        .into_iter()
        .filter(|asset| asset.is_bond())
        .filter_map(|asset| {
            let spec = asset.bond_spec()?;
            spec.valuation_method.is_some().then_some((asset, spec))
        })
        .collect();
    let mut summary = DebtSyncSummary::default();
    if loans.is_empty() {
        return Ok(summary);
    }

    let activities = activity_service.get_activities()?;
    let fixing = |index: &str, on: NaiveDate| {
        fixings
            .fixing_on(index, on)
            .ok()
            .flatten()
            .map(|fixing| fixing.rate)
    };

    for (asset, spec) in &loans {
        let mut accounts: Vec<&str> = activities
            .iter()
            .filter(|a| a.asset_id.as_deref() == Some(asset.id.as_str()))
            .map(|a| a.account_id.as_str())
            .collect();
        accounts.sort_unstable();
        accounts.dedup();

        for account_id in accounts {
            let held: Vec<&Activity> = activities
                .iter()
                .filter(|a| {
                    a.account_id == account_id
                        && a.asset_id.as_deref() == Some(asset.id.as_str())
                        && a.is_posted()
                })
                .collect();
            let Some((principal, start)) = holding(spec, &held) else {
                continue;
            };
            summary.loans += 1;

            let flows = match build_schedule(spec, principal, start, &asset.quote_ccy, fixing) {
                Ok(flows) => flows,
                Err(err) => {
                    warn!(
                        "private_debt: no schedule for {} in {}: {}",
                        asset.id, account_id, err
                    );
                    summary.failures += 1;
                    continue;
                }
            };
            schedules
                .replace_expected_flows(account_id, &asset.id, flows)
                .await?;

            let stored = schedules.list_flows(account_id, &asset.id)?;
            let observed: Vec<ObservedActivity> =
                held.iter().map(|a| ObservedActivity::from(*a)).collect();

            for action in plan_reconciliation(&stored, &observed, today) {
                let result = apply(
                    action,
                    asset,
                    spec,
                    account_id,
                    activity_service.as_ref(),
                    schedules.as_ref(),
                    &mut summary,
                )
                .await;
                if let Err(err) = result {
                    warn!(
                        "private_debt: reconciliation of {} in {} failed: {}",
                        asset.id, account_id, err
                    );
                    summary.failures += 1;
                }
            }
        }
    }

    info!("private_debt: {:?}", summary);
    Ok(summary)
}

/// Principal held and accrual start, from the real purchases of the loan.
fn holding(spec: &BondSpec, held: &[&Activity]) -> Option<(Decimal, NaiveDate)> {
    let purchases: Vec<&&Activity> = held
        .iter()
        .filter(|a| !is_generated(a.idempotency_key.as_deref()))
        .filter(|a| {
            matches!(
                ActivityType::from_str(&a.activity_type),
                Ok(ActivityType::Buy) | Ok(ActivityType::TransferIn)
            )
        })
        .collect();
    let quantity: Decimal = purchases.iter().map(|a| a.qty()).sum();
    let first_purchase = purchases.iter().map(|a| a.effective_date()).min()?;
    let face_value = spec.face_value.unwrap_or(Decimal::ONE);
    let principal = quantity * face_value;
    (principal > Decimal::ZERO).then(|| (principal, spec.issue_date.unwrap_or(first_purchase)))
}

async fn apply(
    action: ReconcileAction,
    asset: &Asset,
    spec: &BondSpec,
    account_id: &str,
    activities: &(dyn ActivityServiceTrait + Send + Sync),
    schedules: &dyn DebtScheduleRepositoryTrait,
    summary: &mut DebtSyncSummary,
) -> Result<()> {
    match action {
        ReconcileAction::Settle {
            flow_id,
            settlement,
            delete_activity,
        } => {
            schedules.settle_flow(&flow_id, settlement).await?;
            if let Some(id) = delete_activity {
                activities.delete_activity(id).await?;
            }
        }
        ReconcileAction::Generate(flow) => {
            let face_value = spec.face_value.unwrap_or(Decimal::ONE);
            let new = |activity_type: &str, key: String, quantity: Option<Decimal>| NewActivity {
                id: None,
                account_id: account_id.to_string(),
                symbol: Some(SymbolInput {
                    id: Some(asset.id.clone()),
                    ..Default::default()
                }),
                activity_type: activity_type.to_string(),
                subtype: None,
                activity_date: flow.flow_date.format("%Y-%m-%d").to_string(),
                quantity,
                unit_price: quantity.map(|_| face_value),
                currency: flow.currency.clone(),
                fee: None,
                amount: Some(flow.expected_amount),
                status: None,
                notes: Some(format!(
                    "Auto-generated from the loan schedule ({} due {})",
                    flow.flow_type.as_str(),
                    flow.flow_date
                )),
                fx_rate: None,
                metadata: None,
                needs_review: None,
                source_system: Some(SOURCE_SYSTEM.to_string()),
                source_record_id: Some(flow.id.clone()),
                source_group_id: None,
                idempotency_key: Some(key),
            };
            let units = Some(flow.expected_amount / face_value);
            let key = generated_key(&flow);
            let created = match flow.flow_type {
                DebtFlowType::InterestCash => new(ACTIVITY_TYPE_INTEREST, key, None),
                DebtFlowType::InterestPik => new(ACTIVITY_TYPE_INTEREST, key, None),
                DebtFlowType::Principal => new(ACTIVITY_TYPE_SELL, key, units),
                DebtFlowType::Fee => new(ACTIVITY_TYPE_FEE, key, None),
            };
            let created = activities.create_activity(created).await?;
            summary.activities_created += 1;
            if flow.flow_type == DebtFlowType::InterestPik {
                let reinvest = format!("{}:REINVEST", generated_key(&flow));
                activities
                    .create_activity(new(ACTIVITY_TYPE_BUY, reinvest, units))
                    .await?;
                summary.activities_created += 1;
            }
            schedules
                .settle_flow(&flow.id, auto_settlement(&flow, created.id))
                .await?;
        }
    }
    summary.flows_settled += 1;
    Ok(())
}

/// What the reconciliation needs to know about a booked activity.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObservedActivity {
    pub id: String,
    pub activity_type: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub idempotency_key: Option<String>,
}

impl From<&Activity> for ObservedActivity {
    fn from(activity: &Activity) -> Self {
        let amount = match activity.amt() {
            amount if amount > Decimal::ZERO => amount,
            _ => activity.qty() * activity.price(),
        };
        Self {
            id: activity.id.clone(),
            activity_type: activity.activity_type.clone(),
            date: activity.effective_date(),
            amount,
            idempotency_key: activity.idempotency_key.clone(),
        }
    }
}

impl ObservedActivity {
    fn is_generated(&self) -> bool {
        is_generated(self.idempotency_key.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReconcileAction {
    /// Record the settlement; `delete_activity` is a generated activity the
    /// settlement supersedes.
    Settle {
        flow_id: String,
        settlement: DebtFlowSettlement,
        delete_activity: Option<String>,
    },
    /// Book the flow as a generated activity.
    Generate(DebtScheduleFlow),
}

/// Decides, for each flow due by `today`, how it is settled.
///
/// - Linked to a real activity: final, nothing to do.
/// - A real activity of the matching kind, dated within the window and not
///   claimed by another flow: settles it (RECEIVED, or PARTIAL when short),
///   superseding any generated activity.
/// - Otherwise an EXPECTED flow gets a generated activity, or is linked to the
///   one already generated for it. A flow whose generated activity was deleted
///   by the user turns LATE and is not generated again.
pub(crate) fn plan_reconciliation(
    flows: &[DebtScheduleFlow],
    activities: &[ObservedActivity],
    today: NaiveDate,
) -> Vec<ReconcileAction> {
    let find = |id: &str| activities.iter().find(|a| a.id == id);
    let mut claimed: HashSet<&str> = flows
        .iter()
        .filter_map(|f| f.activity_id.as_deref())
        .collect();
    let mut actions = Vec::new();

    for flow in flows
        .iter()
        .filter(|f| f.flow_date <= today && f.status != DebtFlowStatus::Waived)
    {
        let current = flow.activity_id.as_deref().and_then(find);
        if current.is_some_and(|a| !a.is_generated()) {
            continue;
        }

        let real = matching_type(flow.flow_type).and_then(|wanted| {
            activities
                .iter()
                .filter(|a| a.activity_type == wanted && !a.is_generated())
                .filter(|a| !claimed.contains(a.id.as_str()))
                .filter(|a| (a.date - flow.flow_date).num_days().abs() <= MATCH_WINDOW_DAYS)
                .min_by_key(|a| {
                    (
                        (a.date - flow.flow_date).num_days().abs(),
                        (a.amount - flow.expected_amount).abs(),
                    )
                })
        });

        if let Some(real) = real {
            claimed.insert(real.id.as_str());
            let status = if real.amount + Decimal::new(1, 2) < flow.expected_amount {
                DebtFlowStatus::Partial
            } else {
                DebtFlowStatus::Received
            };
            actions.push(ReconcileAction::Settle {
                flow_id: flow.id.clone(),
                settlement: DebtFlowSettlement {
                    status,
                    received_amount: Some(real.amount),
                    received_date: Some(real.date),
                    activity_id: Some(real.id.clone()),
                    notes: None,
                },
                delete_activity: current.map(|a| a.id.clone()),
            });
            continue;
        }
        if current.is_some() {
            continue;
        }

        match flow.status {
            DebtFlowStatus::Expected => {
                let key = generated_key(flow);
                match activities
                    .iter()
                    .find(|a| a.idempotency_key.as_deref() == Some(key.as_str()))
                {
                    Some(existing) => actions.push(ReconcileAction::Settle {
                        flow_id: flow.id.clone(),
                        settlement: auto_settlement(flow, existing.id.clone()),
                        delete_activity: None,
                    }),
                    None => actions.push(ReconcileAction::Generate(flow.clone())),
                }
            }
            DebtFlowStatus::Received | DebtFlowStatus::Partial => {
                actions.push(ReconcileAction::Settle {
                    flow_id: flow.id.clone(),
                    settlement: DebtFlowSettlement {
                        status: DebtFlowStatus::Late,
                        received_amount: None,
                        received_date: None,
                        activity_id: None,
                        notes: flow.notes.clone(),
                    },
                    delete_activity: None,
                })
            }
            DebtFlowStatus::Late | DebtFlowStatus::Waived => {}
        }
    }
    actions
}

fn matching_type(flow_type: DebtFlowType) -> Option<&'static str> {
    match flow_type {
        DebtFlowType::InterestCash => Some(ACTIVITY_TYPE_INTEREST),
        DebtFlowType::Principal => Some(ACTIVITY_TYPE_SELL),
        DebtFlowType::Fee => Some(ACTIVITY_TYPE_FEE),
        DebtFlowType::InterestPik => None,
    }
}

fn generated_key(flow: &DebtScheduleFlow) -> String {
    format!(
        "{GENERATED_KEY_PREFIX}{}:{}:{}:{}",
        flow.asset_id,
        flow.account_id,
        flow.flow_date,
        flow.flow_type.as_str()
    )
}

fn is_generated(key: Option<&str>) -> bool {
    key.is_some_and(|k| k.starts_with(GENERATED_KEY_PREFIX))
}

fn auto_settlement(flow: &DebtScheduleFlow, activity_id: String) -> DebtFlowSettlement {
    DebtFlowSettlement {
        status: DebtFlowStatus::Received,
        received_amount: Some(flow.expected_amount),
        received_date: Some(flow.flow_date),
        activity_id: Some(activity_id),
        notes: Some(AUTO_NOTE.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn flow(id: &str, on: &str, flow_type: DebtFlowType, amount: Decimal) -> DebtScheduleFlow {
        DebtScheduleFlow {
            id: id.to_string(),
            account_id: "acc".to_string(),
            asset_id: "loan".to_string(),
            flow_date: date(on),
            flow_type,
            period_start: None,
            expected_amount: amount,
            currency: "EUR".to_string(),
            rate_applied: None,
            principal_before: None,
            status: DebtFlowStatus::Expected,
            received_amount: None,
            received_date: None,
            activity_id: None,
            notes: None,
        }
    }

    fn activity(id: &str, activity_type: &str, on: &str, amount: Decimal) -> ObservedActivity {
        ObservedActivity {
            id: id.to_string(),
            activity_type: activity_type.to_string(),
            date: date(on),
            amount,
            idempotency_key: None,
        }
    }

    fn generated(id: &str, f: &DebtScheduleFlow) -> ObservedActivity {
        ObservedActivity {
            idempotency_key: Some(generated_key(f)),
            ..activity(
                id,
                ACTIVITY_TYPE_INTEREST,
                &f.flow_date.to_string(),
                f.expected_amount,
            )
        }
    }

    const TODAY: &str = "2026-11-19";

    #[test]
    fn real_activities_settle_the_nearest_due_flows() {
        let flows = vec![
            flow("f1", "2026-09-17", DebtFlowType::InterestCash, dec!(1.83)),
            flow("f2", "2026-10-17", DebtFlowType::InterestCash, dec!(1.83)),
            flow("f3", "2026-10-17", DebtFlowType::Principal, dec!(150)),
            flow("f4", "2026-12-17", DebtFlowType::InterestCash, dec!(1.83)),
        ];
        let activities = vec![
            activity("a2", ACTIVITY_TYPE_INTEREST, "2026-10-20", dec!(1.83)),
            activity("a1", ACTIVITY_TYPE_INTEREST, "2026-09-16", dec!(1.50)),
            activity("a3", ACTIVITY_TYPE_SELL, "2026-10-18", dec!(150)),
        ];

        let actions = plan_reconciliation(&flows, &activities, date(TODAY));

        let settled: Vec<_> = actions
            .iter()
            .map(|a| match a {
                ReconcileAction::Settle {
                    flow_id,
                    settlement,
                    ..
                } => (
                    flow_id.as_str(),
                    settlement.activity_id.clone().unwrap(),
                    settlement.status,
                ),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(
            settled,
            vec![
                ("f1", "a1".to_string(), DebtFlowStatus::Partial),
                ("f2", "a2".to_string(), DebtFlowStatus::Received),
                ("f3", "a3".to_string(), DebtFlowStatus::Received),
            ]
        );
    }

    #[test]
    fn missing_flows_are_generated_once() {
        let due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        let actions = plan_reconciliation(&[due.clone()], &[], date(TODAY));
        assert_eq!(actions, vec![ReconcileAction::Generate(due.clone())]);

        // Already generated (e.g. settlement failed last run): link, don't duplicate.
        let actions = plan_reconciliation(&[due.clone()], &[generated("g1", &due)], date(TODAY));
        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, delete_activity: None, .. }]
                if settlement.activity_id.as_deref() == Some("g1")
        ));

        // Linked: nothing left to do.
        let mut linked = due.clone();
        linked.status = DebtFlowStatus::Received;
        linked.activity_id = Some("g1".to_string());
        assert!(plan_reconciliation(&[linked], &[generated("g1", &due)], date(TODAY)).is_empty());
    }

    #[test]
    fn a_real_activity_replaces_the_generated_one() {
        let mut due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        due.status = DebtFlowStatus::Received;
        due.activity_id = Some("g1".to_string());
        let activities = vec![
            generated("g1", &due),
            activity("real", ACTIVITY_TYPE_INTEREST, "2026-11-03", dec!(1.83)),
        ];

        let actions = plan_reconciliation(&[due], &activities, date(TODAY));

        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, delete_activity: Some(deleted), .. }]
                if settlement.activity_id.as_deref() == Some("real") && deleted == "g1"
        ));
    }

    #[test]
    fn a_deleted_generated_activity_turns_the_flow_late() {
        let mut due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        due.status = DebtFlowStatus::Received;

        let actions = plan_reconciliation(&[due.clone()], &[], date(TODAY));
        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, .. }] if settlement.status == DebtFlowStatus::Late
        ));

        due.status = DebtFlowStatus::Late;
        assert!(plan_reconciliation(&[due], &[], date(TODAY)).is_empty());
    }

    #[test]
    fn future_waived_and_out_of_window_flows_are_left_alone() {
        let mut waived = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        waived.status = DebtFlowStatus::Waived;
        let future = flow("f2", "2026-12-01", DebtFlowType::InterestCash, dec!(1.83));
        let actions = plan_reconciliation(
            &[waived, future],
            &[activity(
                "a",
                ACTIVITY_TYPE_INTEREST,
                "2026-11-01",
                dec!(1.83),
            )],
            date(TODAY),
        );
        assert!(actions.is_empty());

        // An interest paid 11 days off is not this flow's.
        let due = flow("f3", "2026-10-01", DebtFlowType::InterestCash, dec!(1.83));
        let actions = plan_reconciliation(
            &[due.clone()],
            &[activity(
                "a",
                ACTIVITY_TYPE_INTEREST,
                "2026-10-12",
                dec!(1.83),
            )],
            date(TODAY),
        );
        assert_eq!(actions, vec![ReconcileAction::Generate(due)]);
    }
}
