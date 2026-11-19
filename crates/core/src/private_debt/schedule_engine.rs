//! Expected cash-flow schedule of a private loan, derived from its terms.
//!
//! Pure computation: the caller supplies the terms ([`BondSpec`]), the principal
//! held, the accrual start and a fixing lookup; nothing is read or stored here.
//!
//! Per period, in order:
//! 1. cash interest on the outstanding principal, at the fixed coupon or at
//!    fixing + margin bounded by floor/cap (floating);
//! 2. PIK interest at `pik_rate`, capitalised into the outstanding principal;
//! 3. principal repayment according to the amortisation type. Whatever is still
//!    outstanding on the last payment date is repaid then.

use chrono::{Datelike, NaiveDate};
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};

use super::private_debt_model::{DebtFlowType, NewDebtScheduleFlow};
use crate::assets::{add_months, months_per_period, BondSpec};
use crate::errors::{Error, Result, ValidationError};

const AMOUNT_DP: u32 = 2;
const MAX_PERIODS: usize = 2000;

/// Builds the full schedule of `principal` held from `start` to maturity.
///
/// `fixing(index_code, date)` returns the index rate (annual, decimal) to apply
/// to a floating period starting on `date`, typically the latest fixing
/// published on or before it.
pub fn build_schedule(
    spec: &BondSpec,
    principal: Decimal,
    start: NaiveDate,
    currency: &str,
    fixing: impl Fn(&str, NaiveDate) -> Option<Decimal>,
) -> Result<Vec<NewDebtScheduleFlow>> {
    if principal <= Decimal::ZERO {
        return Err(invalid("principal must be positive".to_string()));
    }
    let dates = payment_dates(spec, start)?;
    let day_count = spec.day_count_convention.as_deref().unwrap_or("30/360");
    let amortization = spec.amortization_type.as_deref().unwrap_or("BULLET");
    let custom = custom_amortization(spec, &dates, amortization)?;
    let pik_rate = spec.pik_rate.unwrap_or(Decimal::ZERO);

    let flow = |date, flow_type, period_start, amount, rate, principal_before| NewDebtScheduleFlow {
        flow_date: date,
        flow_type,
        period_start,
        expected_amount: amount,
        currency: currency.to_string(),
        rate_applied: rate,
        principal_before: Some(principal_before),
    };

    let mut flows = Vec::new();
    let mut outstanding = principal;
    let mut period_start = start;

    for (i, &date) in dates.iter().enumerate() {
        let remaining = (dates.len() - i) as i64;
        let is_last = remaining == 1;
        let yf = year_fraction(day_count, period_start, date)?;

        let cash_rate = cash_rate(spec, period_start, &fixing)?;
        let interest = round_amount(outstanding * cash_rate * yf);
        if interest > Decimal::ZERO {
            flows.push(flow(
                date,
                DebtFlowType::InterestCash,
                Some(period_start),
                interest,
                Some(cash_rate),
                outstanding,
            ));
        }

        let pik = round_amount(outstanding * pik_rate * yf);
        if pik > Decimal::ZERO {
            flows.push(flow(
                date,
                DebtFlowType::InterestPik,
                Some(period_start),
                pik,
                Some(pik_rate),
                outstanding,
            ));
        }
        let interest_base = outstanding;
        outstanding += pik;

        let repayment = round_amount(if is_last {
            outstanding
        } else {
            match amortization {
                "BULLET" => Decimal::ZERO,
                "LINEAR" => outstanding / Decimal::from(remaining),
                "ANNUITY" => {
                    let r = cash_rate * yf;
                    if r.is_zero() {
                        outstanding / Decimal::from(remaining)
                    } else {
                        let discount = Decimal::ONE / (Decimal::ONE + r).powi(remaining);
                        interest_base * r / (Decimal::ONE - discount) - interest
                    }
                }
                "CUSTOM" => custom
                    .iter()
                    .filter(|(d, _)| *d == date)
                    .map(|(_, fraction)| principal * fraction)
                    .sum(),
                other => return Err(invalid(format!("unknown amortization type {other}"))),
            }
        })
        .clamp(Decimal::ZERO, outstanding);

        if repayment > Decimal::ZERO {
            flows.push(flow(date, DebtFlowType::Principal, None, repayment, None, outstanding));
            outstanding -= repayment;
        }
        period_start = date;
    }

    Ok(flows)
}

/// Payment dates strictly after `start`, up to and including maturity.
///
/// Uses the explicit/derived coupon schedule when there is one; otherwise steps
/// from `start` at the coupon frequency. A ZERO frequency pays once, at maturity.
fn payment_dates(spec: &BondSpec, start: NaiveDate) -> Result<Vec<NaiveDate>> {
    let maturity = spec
        .maturity_date
        .ok_or_else(|| invalid("maturity date is required".to_string()))?;
    if maturity <= start {
        return Err(invalid(format!(
            "maturity {maturity} is not after accrual start {start}"
        )));
    }

    let frequency = spec.coupon_frequency.as_deref().unwrap_or("ZERO");
    let mut dates: Vec<NaiveDate> = if frequency.eq_ignore_ascii_case("ZERO") {
        Vec::new()
    } else {
        let explicit: Vec<NaiveDate> = spec
            .parse_coupon_schedule()
            .into_iter()
            .filter(|d| *d > start && *d <= maturity)
            .collect();
        if !explicit.is_empty() {
            explicit
        } else {
            let months = months_per_period(frequency)
                .ok_or_else(|| invalid(format!("unknown coupon frequency {frequency}")))?;
            (1..)
                .map_while(|i: u32| add_months(start, i * months))
                .take_while(|d| *d < maturity)
                .take(MAX_PERIODS)
                .collect()
        }
    };
    if dates.last() != Some(&maturity) {
        dates.push(maturity);
    }
    dates.sort();
    dates.dedup();
    Ok(dates)
}

fn cash_rate(
    spec: &BondSpec,
    period_start: NaiveDate,
    fixing: &impl Fn(&str, NaiveDate) -> Option<Decimal>,
) -> Result<Decimal> {
    match spec.rate_type.as_deref().unwrap_or("FIXED") {
        "FIXED" => Ok(spec.coupon_rate.unwrap_or(Decimal::ZERO)),
        "FLOATING" => {
            let index = spec
                .rate_index
                .as_deref()
                .ok_or_else(|| invalid("floating loan without rate index".to_string()))?;
            let base = fixing(index, period_start).ok_or_else(|| {
                invalid(format!("no {index} fixing on or before {period_start}"))
            })?;
            let margin = spec.rate_margin_bps.unwrap_or(Decimal::ZERO) / Decimal::from(10_000);
            let mut rate = base + margin;
            if let Some(floor) = spec.rate_floor {
                rate = rate.max(floor);
            }
            if let Some(cap) = spec.rate_cap {
                rate = rate.min(cap);
            }
            Ok(rate)
        }
        other => Err(invalid(format!("unknown rate type {other}"))),
    }
}

/// `(date, fraction of the initial principal)` repayments of a CUSTOM loan.
fn custom_amortization(
    spec: &BondSpec,
    dates: &[NaiveDate],
    amortization: &str,
) -> Result<Vec<(NaiveDate, Decimal)>> {
    if amortization != "CUSTOM" {
        return Ok(Vec::new());
    }
    spec.amortization_schedule
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let parsed = entry.split_once(':').and_then(|(d, f)| {
                Some((
                    NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d").ok()?,
                    f.trim().parse::<Decimal>().ok()?,
                ))
            });
            match parsed {
                Some((date, fraction)) if dates.contains(&date) => Ok((date, fraction)),
                Some((date, _)) => Err(invalid(format!(
                    "amortization date {date} is not a payment date"
                ))),
                None => Err(invalid(format!("invalid amortization entry {entry}"))),
            }
        })
        .collect()
}

/// Year fraction of `[from, to]` under `convention`.
fn year_fraction(convention: &str, from: NaiveDate, to: NaiveDate) -> Result<Decimal> {
    let days = Decimal::from((to - from).num_days());
    match convention {
        // 30E/360: day 31 counts as 30, so every whole month is 30 days.
        "30/360" => {
            let d1 = from.day().min(30) as i64;
            let d2 = to.day().min(30) as i64;
            let n = 360 * (to.year() - from.year()) as i64
                + 30 * (to.month() as i64 - from.month() as i64)
                + (d2 - d1);
            Ok(Decimal::from(n) / Decimal::from(360))
        }
        "ACT/360" => Ok(days / Decimal::from(360)),
        "ACT/365" => Ok(days / Decimal::from(365)),
        // ISDA: each calendar year's share over that year's length.
        "ACT/ACT" => {
            let mut total = Decimal::ZERO;
            let mut cursor = from;
            while cursor < to {
                let next_year = NaiveDate::from_ymd_opt(cursor.year() + 1, 1, 1)
                    .ok_or_else(|| invalid(format!("date out of range: {cursor}")))?;
                let end = next_year.min(to);
                let year_len = if NaiveDate::from_ymd_opt(cursor.year(), 2, 29).is_some() {
                    366
                } else {
                    365
                };
                total += Decimal::from((end - cursor).num_days()) / Decimal::from(year_len);
                cursor = end;
            }
            Ok(total)
        }
        other => Err(invalid(format!("unknown day count convention {other}"))),
    }
}

/// Rounds to cents, half away from zero. Pre-rounding to 10 dp absorbs the
/// representation error of repeating year fractions (150 ? 14.6 % ? 30/360
/// must give 1.825 ? 1.83, not 1.8249? ? 1.82).
fn round_amount(amount: Decimal) -> Decimal {
    amount
        .round_dp(10)
        .round_dp_with_strategy(AMOUNT_DP, RoundingStrategy::MidpointAwayFromZero)
}

fn invalid(message: String) -> Error {
    Error::Validation(ValidationError::InvalidInput(format!(
        "Debt schedule: {message}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn no_fixing(_: &str, _: NaiveDate) -> Option<Decimal> {
        None
    }

    fn monthly(rate: Decimal, maturity: &str) -> BondSpec {
        BondSpec {
            coupon_rate: Some(rate),
            coupon_frequency: Some("MONTHLY".to_string()),
            maturity_date: Some(date(maturity)),
            ..Default::default()
        }
    }

    fn of_type(flows: &[NewDebtScheduleFlow], t: DebtFlowType) -> Vec<Decimal> {
        flows
            .iter()
            .filter(|f| f.flow_type == t)
            .map(|f| f.expected_amount)
            .collect()
    }

    /// VLAD / PJ2262 from the platform export: 150 € at 14.6 %, 14 months,
    /// bought 17.08.2026; the platform shows 1.83 € per monthly payment.
    #[test]
    fn bullet_fixed_loan_matches_the_platform() {
        let spec = monthly(dec!(0.146), "2027-10-17");
        let flows = build_schedule(&spec, dec!(150), date("2026-08-17"), "EUR", no_fixing).unwrap();

        let interest = of_type(&flows, DebtFlowType::InterestCash);
        assert_eq!(interest, vec![dec!(1.83); 14]);
        assert_eq!(flows[0].flow_date, date("2026-09-17"));
        assert_eq!(flows[0].period_start, Some(date("2026-08-17")));

        let last = flows.last().unwrap();
        assert_eq!(
            (last.flow_date, last.flow_type, last.expected_amount),
            (date("2027-10-17"), DebtFlowType::Principal, dec!(150))
        );
    }

    #[test]
    fn floating_rate_applies_margin_floor_and_latest_fixing() {
        let spec = BondSpec {
            rate_type: Some("FLOATING".to_string()),
            rate_index: Some("EURIBOR_3M".to_string()),
            rate_margin_bps: Some(dec!(400)),
            rate_floor: Some(dec!(0.045)),
            coupon_frequency: Some("QUARTERLY".to_string()),
            maturity_date: Some(date("2027-07-01")),
            day_count_convention: Some("ACT/360".to_string()),
            ..Default::default()
        };
        let fixing = |index: &str, on: NaiveDate| {
            assert_eq!(index, "EURIBOR_3M");
            Some(if on < date("2027-01-01") { dec!(0.002) } else { dec!(0.03) })
        };
        let flows = build_schedule(&spec, dec!(1000000), date("2026-07-01"), "EUR", fixing).unwrap();

        let rates: Vec<_> = flows
            .iter()
            .filter(|f| f.flow_type == DebtFlowType::InterestCash)
            .map(|f| f.rate_applied.unwrap())
            .collect();
        assert_eq!(rates, vec![dec!(0.045), dec!(0.045), dec!(0.07), dec!(0.07)]);
        // 1,000,000 × 4.5 % × 92 / 360
        assert_eq!(flows[0].expected_amount, dec!(11500));
    }

    #[test]
    fn floating_rate_without_fixing_is_an_error() {
        let spec = BondSpec {
            rate_type: Some("FLOATING".to_string()),
            rate_index: Some("ESTR".to_string()),
            coupon_frequency: Some("ANNUAL".to_string()),
            maturity_date: Some(date("2028-01-01")),
            ..Default::default()
        };
        assert!(build_schedule(&spec, dec!(100), date("2026-01-01"), "EUR", no_fixing).is_err());
    }

    #[test]
    fn pik_is_capitalised_and_repaid_at_maturity() {
        let spec = BondSpec {
            coupon_rate: Some(dec!(0.05)),
            pik_rate: Some(dec!(0.10)),
            coupon_frequency: Some("ANNUAL".to_string()),
            maturity_date: Some(date("2028-01-01")),
            ..Default::default()
        };
        let flows = build_schedule(&spec, dec!(1000), date("2026-01-01"), "EUR", no_fixing).unwrap();

        assert_eq!(of_type(&flows, DebtFlowType::InterestCash), vec![dec!(50), dec!(55)]);
        assert_eq!(of_type(&flows, DebtFlowType::InterestPik), vec![dec!(100), dec!(110)]);
        assert_eq!(of_type(&flows, DebtFlowType::Principal), vec![dec!(1210)]);
    }

    #[test]
    fn linear_and_annuity_repay_the_whole_principal() {
        let mut spec = monthly(dec!(0.12), "2027-01-01");
        spec.amortization_type = Some("LINEAR".to_string());
        let linear = build_schedule(&spec, dec!(1200), date("2026-01-01"), "EUR", no_fixing).unwrap();
        assert_eq!(of_type(&linear, DebtFlowType::Principal), vec![dec!(100); 12]);

        spec.amortization_type = Some("ANNUITY".to_string());
        let annuity = build_schedule(&spec, dec!(1200), date("2026-01-01"), "EUR", no_fixing).unwrap();
        let principal: Decimal = of_type(&annuity, DebtFlowType::Principal).iter().sum();
        assert_eq!(principal, dec!(1200));
        // Constant instalment: 1200 at 1 % a month over 12 months = 106.62.
        let mut instalments = std::collections::BTreeMap::new();
        for f in &annuity {
            *instalments.entry(f.flow_date).or_insert(Decimal::ZERO) += f.expected_amount;
        }
        let first_eleven: Vec<_> = instalments.values().take(11).copied().collect();
        assert!(first_eleven.iter().all(|v| (*v - dec!(106.62)).abs() <= dec!(0.01)));
    }

    #[test]
    fn custom_amortization_repays_fractions_then_the_rest() {
        let spec = BondSpec {
            coupon_rate: Some(dec!(0.08)),
            coupon_frequency: Some("SEMI_ANNUAL".to_string()),
            maturity_date: Some(date("2028-01-01")),
            amortization_type: Some("CUSTOM".to_string()),
            amortization_schedule: Some("2027-01-01:0.25, 2027-07-01:0.25".to_string()),
            ..Default::default()
        };
        let flows = build_schedule(&spec, dec!(1000), date("2026-01-01"), "EUR", no_fixing).unwrap();
        assert_eq!(
            of_type(&flows, DebtFlowType::Principal),
            vec![dec!(250), dec!(250), dec!(500)]
        );
        // Interest follows the amortised principal.
        assert_eq!(
            of_type(&flows, DebtFlowType::InterestCash),
            vec![dec!(40), dec!(40), dec!(30), dec!(20)]
        );
    }

    #[test]
    fn thirty_360_counts_month_ends_as_thirty_days() {
        assert_eq!(
            year_fraction("30/360", date("2026-01-31"), date("2026-03-31")).unwrap(),
            dec!(60) / dec!(360)
        );
        assert_eq!(
            year_fraction("ACT/ACT", date("2027-07-01"), date("2028-07-01")).unwrap(),
            dec!(184) / dec!(365) + dec!(182) / dec!(366)
        );
    }
}
