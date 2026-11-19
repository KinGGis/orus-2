//! Daily price of a private loan, per unit held, from its schedule.
//!
//! The price is dirty: the clean value given by the loan's `valuation_method`
//! plus the interest accrued since the start of the current period, so the
//! position grows day by day and drops back when the interest is paid out.
//! Once a flow of the loan is LATE, interest stops accruing from that flow's
//! date: the loan is carried at its clean value.
//!
//! Clean value per unit (one unit = one face value of principal):
//! - PAR: the face value;
//! - AMORTIZED_COST: the average purchase price, moved linearly to par by
//!   maturity;
//! - YTM: the future cash flows of the schedule discounted at
//!   `ytm_fixed_value` (actual/365), less the accrued interest.
//!
//! MANUAL loans are not priced: their quotes are entered by the user.

use chrono::NaiveDate;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use super::private_debt_model::{DebtFlowStatus, DebtFlowType, DebtScheduleFlow};
use crate::assets::BondSpec;
use crate::errors::{Error, Result, ValidationError};

const PRICE_DP: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValuationMethod {
    Par,
    AmortizedCost,
    Ytm,
    Manual,
}

impl ValuationMethod {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "PAR" => Ok(Self::Par),
            "AMORTIZED_COST" => Ok(Self::AmortizedCost),
            "YTM" => Ok(Self::Ytm),
            "MANUAL" => Ok(Self::Manual),
            other => Err(invalid(format!("unknown valuation method {other}"))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Par => "PAR",
            Self::AmortizedCost => "AMORTIZED_COST",
            Self::Ytm => "YTM",
            Self::Manual => "MANUAL",
        }
    }
}

/// Average price paid per unit and date of the first purchase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Purchase {
    pub price: Decimal,
    pub date: NaiveDate,
}

/// Price per unit for every day from the first purchase to `today`.
pub(crate) fn price_series(
    method: ValuationMethod,
    spec: &BondSpec,
    flows: &[DebtScheduleFlow],
    purchase: Purchase,
    today: NaiveDate,
) -> Result<Vec<(NaiveDate, Decimal)>> {
    let ytm = match method {
        ValuationMethod::Manual => return Ok(Vec::new()),
        ValuationMethod::Ytm => Some(
            spec.ytm_fixed_value
                .ok_or_else(|| invalid("YTM valuation without ytm_fixed_value".to_string()))?,
        ),
        _ => None,
    };
    let face = spec.face_value.unwrap_or(Decimal::ONE);
    let late_from = flows
        .iter()
        .filter(|f| f.status == DebtFlowStatus::Late)
        .map(|f| f.flow_date)
        .min();

    let price_on = |day: NaiveDate| -> Result<Decimal> {
        let accrued = accrued(flows, face, day);
        let clean = match method {
            ValuationMethod::Par => face,
            ValuationMethod::AmortizedCost => amortized_cost(spec, face, purchase, day),
            ValuationMethod::Ytm => match units(flows, face, day) {
                Some(units) => {
                    present_value(flows, ytm.unwrap_or_default(), day)? / units - accrued
                }
                None => face,
            },
            ValuationMethod::Manual => unreachable!(),
        };
        let accruing = late_from.is_none_or(|late| day < late);
        let price = if accruing { clean + accrued } else { clean };
        Ok(price.round_dp(PRICE_DP))
    };

    purchase
        .date
        .iter_days()
        .take_while(|day| *day <= today)
        .map(|day| Ok((day, price_on(day)?)))
        .collect()
}

/// Interest accrued per unit on `day` over the periods in progress, linearly
/// between their start and payment dates. PIK interest accrues the same way.
fn accrued(flows: &[DebtScheduleFlow], face: Decimal, day: NaiveDate) -> Decimal {
    flows
        .iter()
        .filter(|f| {
            matches!(
                f.flow_type,
                DebtFlowType::InterestCash | DebtFlowType::InterestPik
            )
        })
        .filter_map(|f| {
            let start = f.period_start?;
            let principal = f.principal_before.filter(|p| *p > Decimal::ZERO)?;
            if day < start || day >= f.flow_date {
                return None;
            }
            let elapsed = Decimal::from((day - start).num_days());
            let length = Decimal::from((f.flow_date - start).num_days());
            Some(f.expected_amount * face / principal * elapsed / length)
        })
        .sum()
}

/// Average purchase price, moved linearly to par between the first purchase
/// and maturity.
fn amortized_cost(spec: &BondSpec, face: Decimal, purchase: Purchase, day: NaiveDate) -> Decimal {
    let Some(maturity) = spec.maturity_date.filter(|m| *m > purchase.date) else {
        return purchase.price;
    };
    let elapsed = Decimal::from((day.min(maturity) - purchase.date).num_days().max(0));
    let length = Decimal::from((maturity - purchase.date).num_days());
    purchase.price + (face - purchase.price) * elapsed / length
}

/// Units outstanding on `day` per the schedule: principal of the period in
/// progress, divided by the face value. None once the schedule has run out.
fn units(flows: &[DebtScheduleFlow], face: Decimal, day: NaiveDate) -> Option<Decimal> {
    let next = flows
        .iter()
        .filter(|f| f.flow_date > day)
        .map(|f| f.flow_date)
        .min()?;
    let on_next = || flows.iter().filter(move |f| f.flow_date == next);
    on_next()
        .find(|f| f.flow_type != DebtFlowType::Principal)
        .or_else(|| on_next().next())
        .and_then(|f| f.principal_before)
        .filter(|p| *p > Decimal::ZERO)
        .map(|p| p / face)
}

/// Cash interest and principal still to come after `day`, discounted at `ytm`.
fn present_value(flows: &[DebtScheduleFlow], ytm: Decimal, day: NaiveDate) -> Result<Decimal> {
    let rate = ytm
        .to_f64()
        .filter(|r| *r > -1.0)
        .ok_or_else(|| invalid(format!("invalid ytm {ytm}")))?;
    flows
        .iter()
        .filter(|f| f.flow_date > day)
        .filter(|f| {
            matches!(
                f.flow_type,
                DebtFlowType::InterestCash | DebtFlowType::Principal
            )
        })
        .map(|f| {
            let years = (f.flow_date - day).num_days() as f64 / 365.0;
            let discount = Decimal::from_f64((1.0 + rate).powf(-years))
                .ok_or_else(|| invalid(format!("cannot discount at ytm {ytm}")))?;
            Ok(f.expected_amount * discount)
        })
        .sum()
}

fn invalid(message: String) -> Error {
    Error::Validation(ValidationError::InvalidInput(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn flow(
        on: &str,
        flow_type: DebtFlowType,
        period_start: Option<&str>,
        amount: Decimal,
        principal_before: Decimal,
    ) -> DebtScheduleFlow {
        DebtScheduleFlow {
            id: format!("{on}:{}", flow_type.as_str()),
            account_id: "acc".to_string(),
            asset_id: "loan".to_string(),
            flow_date: date(on),
            flow_type,
            period_start: period_start.map(date),
            expected_amount: amount,
            currency: "EUR".to_string(),
            rate_applied: None,
            principal_before: Some(principal_before),
            status: DebtFlowStatus::Expected,
            received_amount: None,
            received_date: None,
            activity_id: None,
            notes: None,
        }
    }

    /// 1,000 lent on 2026-01-01 at 12 %, interest every 30 days, repaid
    /// half on 2026-01-31 and the rest on 2026-03-02.
    fn amortizing_loan() -> Vec<DebtScheduleFlow> {
        vec![
            flow(
                "2026-01-31",
                DebtFlowType::InterestCash,
                Some("2026-01-01"),
                dec!(10),
                dec!(1000),
            ),
            flow(
                "2026-01-31",
                DebtFlowType::Principal,
                None,
                dec!(500),
                dec!(1000),
            ),
            flow(
                "2026-03-02",
                DebtFlowType::InterestCash,
                Some("2026-01-31"),
                dec!(5),
                dec!(500),
            ),
            flow(
                "2026-03-02",
                DebtFlowType::Principal,
                None,
                dec!(500),
                dec!(500),
            ),
        ]
    }

    fn spec(method_fields: impl FnOnce(&mut BondSpec)) -> BondSpec {
        let mut spec = BondSpec {
            maturity_date: Some(date("2026-03-02")),
            ..Default::default()
        };
        method_fields(&mut spec);
        spec
    }

    fn at_par(on: &str) -> Purchase {
        Purchase {
            price: Decimal::ONE,
            date: date(on),
        }
    }

    fn price(series: &[(NaiveDate, Decimal)], on: &str) -> Decimal {
        series.iter().find(|(d, _)| *d == date(on)).unwrap().1
    }

    #[test]
    fn par_accrues_interest_daily_and_resets_on_payment() {
        let series = price_series(
            ValuationMethod::Par,
            &spec(|_| {}),
            &amortizing_loan(),
            at_par("2026-01-01"),
            date("2026-02-15"),
        )
        .unwrap();

        assert_eq!(series.len(), 46);
        assert_eq!(price(&series, "2026-01-01"), dec!(1));
        // 15 of 30 days of 1 % per period
        assert_eq!(price(&series, "2026-01-16"), dec!(1.005));
        assert_eq!(price(&series, "2026-01-31"), dec!(1));
        // second period: 5 on 500 outstanding, still 1 % per unit
        assert_eq!(price(&series, "2026-02-15"), dec!(1.005));
    }

    #[test]
    fn a_late_flow_stops_the_accrual() {
        let mut flows = amortizing_loan();
        flows[0].status = DebtFlowStatus::Late;

        let series = price_series(
            ValuationMethod::Par,
            &spec(|_| {}),
            &flows,
            at_par("2026-01-01"),
            date("2026-02-15"),
        )
        .unwrap();

        assert_eq!(price(&series, "2026-01-16"), dec!(1.005));
        assert_eq!(price(&series, "2026-01-31"), dec!(1));
        assert_eq!(price(&series, "2026-02-15"), dec!(1));
    }

    #[test]
    fn amortized_cost_pulls_a_discount_to_par_by_maturity() {
        let purchase = Purchase {
            price: dec!(0.94),
            date: date("2026-01-01"),
        };
        let series = price_series(
            ValuationMethod::AmortizedCost,
            &spec(|_| {}),
            &amortizing_loan(),
            purchase,
            date("2026-03-02"),
        )
        .unwrap();

        assert_eq!(price(&series, "2026-01-01"), dec!(0.94));
        // half-way through the 60 days, on a payment date (nothing accrued)
        assert_eq!(price(&series, "2026-01-31"), dec!(0.97));
        assert_eq!(price(&series, "2026-03-02"), dec!(1));
    }

    #[test]
    fn ytm_discounts_the_remaining_flows_per_unit_outstanding() {
        let flows = amortizing_loan();
        let at_coupon = spec(|s| s.ytm_fixed_value = Some(dec!(0)));
        let series = price_series(
            ValuationMethod::Ytm,
            &at_coupon,
            &flows,
            at_par("2026-01-01"),
            date("2026-02-15"),
        )
        .unwrap();
        // undiscounted: (10 + 500 + 5 + 500) / 1000 units
        assert_eq!(price(&series, "2026-01-01"), dec!(1.015));
        // after the first payment: (5 + 500) / 500 units
        assert_eq!(price(&series, "2026-01-31"), dec!(1.01));

        let above_coupon = spec(|s| s.ytm_fixed_value = Some(dec!(0.20)));
        let series = price_series(
            ValuationMethod::Ytm,
            &above_coupon,
            &flows,
            at_par("2026-01-01"),
            date("2026-01-01"),
        )
        .unwrap();
        assert!(price(&series, "2026-01-01") < dec!(1));
    }

    #[test]
    fn ytm_requires_a_yield_and_manual_is_not_priced() {
        let flows = amortizing_loan();
        assert!(price_series(
            ValuationMethod::Ytm,
            &spec(|_| {}),
            &flows,
            at_par("2026-01-01"),
            date("2026-01-10")
        )
        .is_err());
        assert!(price_series(
            ValuationMethod::Manual,
            &spec(|_| {}),
            &flows,
            at_par("2026-01-01"),
            date("2026-01-10")
        )
        .unwrap()
        .is_empty());
    }
}
