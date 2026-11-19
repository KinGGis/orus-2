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
//! 3. a due flow with no real counterpart yet is booked as an ESTIMATED
//!    generated activity, counted in the valuation until the monthly platform
//!    statement is imported: a real activity then replaces it (CONFIRMED), and
//!    if the statement covers the flow's date without paying it, the estimate
//!    is removed and the flow turns LATE. Future flows stay projections in the
//!    schedule. PIK interest is booked as INTEREST reinvested through a BUY of
//!    the same amount.
//!
//! The loan is then priced daily from its schedule (see [`super::valuation`]),
//! from the first account holding it. Its quotes are stored under the
//! INTERNAL_BOND_MODEL source, and its quote mode is kept MANUAL so that no
//! market provider prices it: a quote entered by the user on a given day
//! takes precedence over the computed one.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{NaiveDate, TimeZone, Utc};
use log::{info, warn};
use rust_decimal::Decimal;

use super::private_debt_model::{
    DebtFlowSettlement, DebtFlowStatus, DebtFlowType, DebtScheduleFlow,
};
use super::private_debt_traits::{DebtScheduleRepositoryTrait, RateFixingRepositoryTrait};
use super::schedule_engine::build_schedule;
use super::valuation::{price_series, Purchase, ValuationMethod};
use crate::activities::{
    Activity, ActivityServiceTrait, ActivityType, NewActivity, SymbolInput, ACTIVITY_TYPE_BUY,
    ACTIVITY_TYPE_FEE, ACTIVITY_TYPE_INTEREST, ACTIVITY_TYPE_SELL,
};
use crate::assets::{Asset, AssetServiceTrait, BondSpec, QuoteMode};
use crate::errors::Result;
use crate::quotes::{DataSource, Quote, QuoteServiceTrait};

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
    pub quotes_written: usize,
    pub failures: usize,
}

pub async fn sync_private_debt(
    asset_service: Arc<dyn AssetServiceTrait + Send + Sync>,
    activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
    quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
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

    let real = |a: &&Activity| a.is_posted() && !is_generated(a.idempotency_key.as_deref());

    for (asset, spec) in &loans {
        if asset.quote_mode != QuoteMode::Manual {
            if let Err(err) = asset_service
                .update_quote_mode_silent(&asset.id, QuoteMode::Manual.as_db_str())
                .await
            {
                warn!(
                    "private_debt: cannot set {} to manual quotes: {}",
                    asset.id, err
                );
            }
        }
        let mut priced = false;

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
            let statement = statement_coverage(
                activities
                    .iter()
                    .filter(|a| a.account_id == account_id)
                    .filter(real)
                    .map(|a| a.effective_date()),
            );

            for action in plan_reconciliation(&stored, &observed, statement, today) {
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

            if !priced {
                priced = true;
                let result = price_loan(
                    asset,
                    spec,
                    &schedules.list_flows(account_id, &asset.id)?,
                    purchase(spec, &held, start),
                    today,
                    quote_service.as_ref(),
                )
                .await;
                match result {
                    Ok(written) => summary.quotes_written += written,
                    Err(err) => {
                        warn!("private_debt: valuation of {} failed: {}", asset.id, err);
                        summary.failures += 1;
                    }
                }
            }
        }
    }

    info!("private_debt: {:?}", summary);
    Ok(summary)
}

/// Average price paid per unit for the loan, and date of the first purchase.
/// Purchases without a price count at par.
fn purchase(spec: &BondSpec, held: &[&Activity], start: NaiveDate) -> Purchase {
    let face_value = spec.face_value.unwrap_or(Decimal::ONE);
    let (quantity, cost, first) = held
        .iter()
        .filter(|a| !is_generated(a.idempotency_key.as_deref()))
        .filter(|a| {
            matches!(
                ActivityType::from_str(&a.activity_type),
                Ok(ActivityType::Buy) | Ok(ActivityType::TransferIn)
            )
        })
        .fold((Decimal::ZERO, Decimal::ZERO, None), |(q, c, first), a| {
            let price = Some(a.price())
                .filter(|p| *p > Decimal::ZERO)
                .unwrap_or(face_value);
            let date = a.effective_date();
            let first = Some(first.map_or(date, |f: NaiveDate| f.min(date)));
            (q + a.qty(), c + a.qty() * price, first)
        });
    Purchase {
        price: if quantity > Decimal::ZERO {
            cost / quantity
        } else {
            face_value
        },
        date: first.unwrap_or(start),
    }
}

/// Writes the computed prices of the loan that changed, and removes the
/// computed quotes it no longer has. Returns the number of quotes written.
async fn price_loan(
    asset: &Asset,
    spec: &BondSpec,
    flows: &[DebtScheduleFlow],
    purchase: Purchase,
    today: NaiveDate,
    quotes: &(dyn QuoteServiceTrait + Send + Sync),
) -> Result<usize> {
    let method = ValuationMethod::parse(spec.valuation_method.as_deref().unwrap_or_default())?;
    let series = price_series(method, spec, flows, purchase, today)?;

    let existing: HashMap<NaiveDate, (String, Decimal)> = quotes
        .get_historical_quotes(&asset.id)?
        .into_iter()
        .filter(|q| q.data_source == DataSource::InternalBondModel)
        .map(|q| (q.timestamp.date_naive(), (q.id, q.close)))
        .collect();
    let days: HashSet<NaiveDate> = series.iter().map(|(day, _)| *day).collect();

    let changed: Vec<Quote> = series
        .into_iter()
        .filter(|(day, price)| existing.get(day).is_none_or(|(_, close)| close != price))
        .map(|(day, price)| loan_quote(asset, method, day, price))
        .collect();
    let written = changed.len();
    if written > 0 {
        quotes.bulk_upsert_quotes(changed).await?;
    }
    for (id, _) in existing
        .into_iter()
        .filter(|(day, _)| !days.contains(day))
        .map(|(_, quote)| quote)
    {
        quotes.delete_quote(&id).await?;
    }
    Ok(written)
}

fn loan_quote(asset: &Asset, method: ValuationMethod, day: NaiveDate, price: Decimal) -> Quote {
    let timestamp = day
        .and_hms_opt(16, 0, 0)
        .map(|dt| Utc.from_utc_datetime(&dt))
        .unwrap_or_else(Utc::now);
    Quote {
        id: format!(
            "{}_{}_{}",
            asset.id,
            day,
            DataSource::InternalBondModel.as_str()
        ),
        asset_id: asset.id.clone(),
        timestamp,
        open: price,
        high: price,
        low: price,
        close: price,
        adjclose: price,
        volume: Decimal::ZERO,
        currency: asset.quote_ccy.clone(),
        data_source: DataSource::InternalBondModel,
        created_at: Utc::now(),
        notes: Some(format!("Private debt valuation ({})", method.as_str())),
    }
}

/// Period the imported statements cover: from the first to the last real
/// activity booked in the account.
fn statement_coverage(dates: impl Iterator<Item = NaiveDate>) -> Option<(NaiveDate, NaiveDate)> {
    dates.fold(None, |range, d| match range {
        None => Some((d, d)),
        Some((from, to)) => Some((from.min(d), to.max(d))),
    })
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
                .settle_flow(&flow.id, estimated_settlement(&flow, created.id))
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
///   claimed by another flow: CONFIRMED (PARTIAL when short), superseding the
///   estimate.
/// - Otherwise, once the imported statement covers the flow's date (beyond the
///   window), the flow was not paid: LATE, and its estimate is removed.
/// - Otherwise an EXPECTED flow is booked as ESTIMATED (generated activity, or
///   the one already generated for it). A flow whose activity was deleted by
///   the user turns LATE and is not generated again.
pub(crate) fn plan_reconciliation(
    flows: &[DebtScheduleFlow],
    activities: &[ObservedActivity],
    statement: Option<(NaiveDate, NaiveDate)>,
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
                DebtFlowStatus::Confirmed
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
        let late = |delete_activity: Option<String>| ReconcileAction::Settle {
            flow_id: flow.id.clone(),
            settlement: DebtFlowSettlement {
                status: DebtFlowStatus::Late,
                received_amount: None,
                received_date: None,
                activity_id: None,
                notes: flow.notes.clone(),
            },
            delete_activity,
        };
        let unpaid = flow.flow_type != DebtFlowType::InterestPik
            && statement.is_some_and(|(from, to)| {
                from <= flow.flow_date && (to - flow.flow_date).num_days() > MATCH_WINDOW_DAYS
            });

        if let Some(estimate) = current {
            if unpaid {
                actions.push(late(Some(estimate.id.clone())));
            }
            continue;
        }

        match flow.status {
            DebtFlowStatus::Expected => {
                let key = generated_key(flow);
                let existing = activities
                    .iter()
                    .find(|a| a.idempotency_key.as_deref() == Some(key.as_str()));
                match existing {
                    _ if unpaid => actions.push(late(existing.map(|a| a.id.clone()))),
                    Some(existing) => actions.push(ReconcileAction::Settle {
                        flow_id: flow.id.clone(),
                        settlement: estimated_settlement(flow, existing.id.clone()),
                        delete_activity: None,
                    }),
                    None => actions.push(ReconcileAction::Generate(flow.clone())),
                }
            }
            DebtFlowStatus::Estimated | DebtFlowStatus::Confirmed | DebtFlowStatus::Partial => {
                actions.push(late(None))
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

fn estimated_settlement(flow: &DebtScheduleFlow, activity_id: String) -> DebtFlowSettlement {
    DebtFlowSettlement {
        status: DebtFlowStatus::Estimated,
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

        let actions = plan_reconciliation(&flows, &activities, None, date(TODAY));

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
                ("f2", "a2".to_string(), DebtFlowStatus::Confirmed),
                ("f3", "a3".to_string(), DebtFlowStatus::Confirmed),
            ]
        );
    }

    #[test]
    fn missing_flows_are_generated_once() {
        let due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        let actions = plan_reconciliation(&[due.clone()], &[], None, date(TODAY));
        assert_eq!(actions, vec![ReconcileAction::Generate(due.clone())]);

        // Already generated (e.g. settlement failed last run): link, don't duplicate.
        let actions =
            plan_reconciliation(&[due.clone()], &[generated("g1", &due)], None, date(TODAY));
        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, delete_activity: None, .. }]
                if settlement.activity_id.as_deref() == Some("g1")
        ));

        // Linked: nothing left to do.
        let mut linked = due.clone();
        linked.status = DebtFlowStatus::Estimated;
        linked.activity_id = Some("g1".to_string());
        assert!(
            plan_reconciliation(&[linked], &[generated("g1", &due)], None, date(TODAY)).is_empty()
        );
    }

    #[test]
    fn a_real_activity_replaces_the_generated_one() {
        let mut due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        due.status = DebtFlowStatus::Estimated;
        due.activity_id = Some("g1".to_string());
        let activities = vec![
            generated("g1", &due),
            activity("real", ACTIVITY_TYPE_INTEREST, "2026-11-03", dec!(1.83)),
        ];

        let actions = plan_reconciliation(&[due], &activities, None, date(TODAY));

        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, delete_activity: Some(deleted), .. }]
                if settlement.activity_id.as_deref() == Some("real") && deleted == "g1"
        ));
    }

    #[test]
    fn a_deleted_generated_activity_turns_the_flow_late() {
        let mut due = flow("f1", "2026-11-01", DebtFlowType::InterestCash, dec!(1.83));
        due.status = DebtFlowStatus::Estimated;

        let actions = plan_reconciliation(&[due.clone()], &[], None, date(TODAY));
        assert!(matches!(
            &actions[..],
            [ReconcileAction::Settle { settlement, .. }] if settlement.status == DebtFlowStatus::Late
        ));

        due.status = DebtFlowStatus::Late;
        assert!(plan_reconciliation(&[due], &[], None, date(TODAY)).is_empty());
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
            None,
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
            None,
            date(TODAY),
        );
        assert_eq!(actions, vec![ReconcileAction::Generate(due)]);
    }

    #[test]
    fn a_statement_covering_an_unpaid_flow_turns_it_late() {
        let mut estimated = flow("f1", "2026-10-01", DebtFlowType::InterestCash, dec!(1.83));
        estimated.status = DebtFlowStatus::Estimated;
        estimated.activity_id = Some("g1".to_string());
        let unbooked = flow("f2", "2026-10-15", DebtFlowType::InterestCash, dec!(1.83));
        let mut pik = flow("f3", "2026-10-01", DebtFlowType::InterestPik, dec!(0.5));
        pik.status = DebtFlowStatus::Estimated;
        pik.activity_id = Some("g3".to_string());
        let flows = [estimated.clone(), unbooked, pik.clone()];
        let activities = [generated("g1", &estimated), generated("g3", &pik)];

        // Statement not imported past the flows yet: estimates stand.
        let partial = Some((date("2026-01-01"), date("2026-10-10")));
        let actions = plan_reconciliation(&flows, &activities, partial, date(TODAY));
        assert_eq!(actions, vec![ReconcileAction::Generate(flows[1].clone())]);

        // Statement covers them without paying: late, estimate removed. PIK
        // has no cash counterpart and stays estimated.
        let covered = Some((date("2026-01-01"), date("2026-11-15")));
        let actions = plan_reconciliation(&flows, &activities, covered, date(TODAY));
        let late: Vec<_> = actions
            .iter()
            .map(|a| match a {
                ReconcileAction::Settle {
                    flow_id,
                    settlement,
                    delete_activity,
                } => {
                    assert_eq!(settlement.status, DebtFlowStatus::Late);
                    (flow_id.as_str(), delete_activity.clone())
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(late, vec![("f1", Some("g1".to_string())), ("f2", None)]);

        // A flow before the first imported activity is not judged.
        let later = Some((date("2026-10-10"), date("2026-11-15")));
        let actions = plan_reconciliation(&flows[..1], &activities, later, date(TODAY));
        assert!(actions.is_empty());
    }
}
