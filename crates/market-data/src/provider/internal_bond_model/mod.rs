//! Internal YTM bond pricing provider.
//!
//! Computes a daily dirty (or clean) price for bonds whose `quote_mode` is
//! `INTERNAL_YTM`.  No external network call is made: all parameters come from
//! the bond's `BondSpec` (stored in `Asset.metadata["bond"]`).
//!
//! # Pricing algorithm
//!
//! 1. Settlement date = evaluation date + `settlement_days` calendar days.
//! 2. Build the list of future coupon cash flows from `coupon_schedule`.
//! 3. Discount each cash flow with the stored annual YTM using the chosen
//!    day-count convention:
//!    ```
//!    PV(CF_i) = CF_i / (1 + ytm) ^ year_fraction(settlement, coupon_date_i)
//!    ```
//! 4. Sum the coupon PVs and add the discounted face-value redemption.
//! 5. That sum is the **clean price**.
//! 6. Compute accrued interest from the last coupon date to settlement, skipping
//!    the ex-coupon window if applicable.
//! 7. Return `clean + accrued` (dirty) or `clean` depending on `pricing_method`.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use tracing::{debug, warn};

use crate::errors::MarketDataError;
use crate::models::{Coverage, InstrumentKind, ProviderInstrument, Quote, QuoteContext};
use crate::models::{DayCountConvention, PricingMethod, YtmSource};
use crate::provider::{MarketDataProvider, ProviderCapabilities, RateLimit};

pub const PROVIDER_ID: &str = "INTERNAL_BOND_MODEL";

// ---------------------------------------------------------------------------
// Provider struct
// ---------------------------------------------------------------------------

/// Market-data provider that computes bond prices from stored YTM parameters.
///
/// Register this provider alongside the market-data providers.  It will only
/// be invoked for bonds whose `quote_context.bond_ytm_params` is `Some(…)`.
pub struct InternalBondPricingProvider;

// ---------------------------------------------------------------------------
// Core pricing functions (pub(crate) so they can be unit-tested)
// ---------------------------------------------------------------------------

/// Calculate the **clean price** of a bond.
///
/// Returns the price in the same unit as `face_value` (not fraction-of-par).
pub(crate) fn calculate_clean_price(
    ytm: f64,
    settlement: NaiveDate,
    maturity_date: NaiveDate,
    coupon_rate: f64,
    face_value: f64,
    coupon_schedule: &[NaiveDate],
    day_count: DayCountConvention,
) -> Result<f64, MarketDataError> {
    // Bond already matured → return face value
    if settlement >= maturity_date {
        return Ok(face_value);
    }

    let mut pv = 0.0_f64;

    // Sum PV of future coupon payments
    for &coupon_date in coupon_schedule {
        if coupon_date <= settlement {
            continue;
        }
        let t = day_count.year_fraction(settlement, coupon_date);
        let discount = (1.0 + ytm).powf(t);
        // Coupon amount = annual coupon rate × face / periods_per_year
        // We derive periods_per_year from the schedule itself for robustness
        let coupon = coupon_payment(coupon_rate, face_value, coupon_schedule);
        pv += coupon / discount;
    }

    // PV of redemption at maturity
    let t_mat = day_count.year_fraction(settlement, maturity_date);
    let discount_mat = (1.0 + ytm).powf(t_mat);
    pv += face_value / discount_mat;

    if pv.is_nan() || pv.is_infinite() {
        return Err(MarketDataError::ProviderError {
            provider: PROVIDER_ID.to_string(),
            message: "Clean price calculation resulted in NaN/Inf".to_string(),
        });
    }

    Ok(pv)
}

/// Compute accrued interest on `settlement` using the coupon schedule.
///
/// Returns 0 if settlement falls in the ex-coupon window (within `ex_coupon_days`
/// before the next coupon payment).
pub(crate) fn calculate_accrued_interest(
    coupon_rate: f64,
    face_value: f64,
    settlement: NaiveDate,
    coupon_schedule: &[NaiveDate],
    _day_count: DayCountConvention,
    ex_coupon_days: i32,
) -> f64 {
    // Find the last coupon date ≤ settlement
    let last_coupon = coupon_schedule.iter().filter(|&&d| d <= settlement).max();
    // Find the next coupon date > settlement
    let next_coupon = coupon_schedule.iter().find(|&&d| d > settlement);

    let (last, next) = match (last_coupon, next_coupon) {
        (Some(&l), Some(&n)) => (l, n),
        _ => return 0.0, // Can't compute without both boundaries
    };

    // Ex-coupon check: no accrual in the window before next coupon
    let days_to_next = (next - settlement).num_days();
    if ex_coupon_days > 0 && days_to_next <= ex_coupon_days as i64 {
        return 0.0;
    }

    let days_accrued = (settlement - last).num_days() as f64;
    let period_days = (next - last).num_days() as f64;
    if period_days == 0.0 {
        return 0.0;
    }

    let coupon = coupon_payment(coupon_rate, face_value, coupon_schedule);
    coupon * (days_accrued / period_days)
}

/// Derive the cash-flow amount per coupon from the coupon schedule length.
///
/// We infer the number of periods per year from the median gap between
/// consecutive coupon dates.  Falls back to semi-annual (2) if the schedule
/// has fewer than 2 entries.
fn coupon_payment(coupon_rate: f64, face_value: f64, schedule: &[NaiveDate]) -> f64 {
    let periods_per_year = infer_periods_per_year(schedule).max(1) as f64;
    (coupon_rate * face_value) / periods_per_year
}

/// Estimate periods-per-year from the median coupon gap.
fn infer_periods_per_year(schedule: &[NaiveDate]) -> u32 {
    if schedule.len() < 2 {
        return 2; // default semi-annual
    }
    let mut gaps: Vec<i64> = schedule
        .windows(2)
        .map(|w| (w[1] - w[0]).num_days())
        .collect();
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    // Map typical coupon gaps to periods per year
    match median {
        ..=35 => 12,   // monthly
        36..=100 => 4, // quarterly
        101..=200 => 2, // semi-annual
        _ => 1,         // annual
    }
}

// ---------------------------------------------------------------------------
// Provider trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl MarketDataProvider for InternalBondPricingProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    /// Lower priority number = higher priority in the registry.
    /// We give this provider a very high priority (2) so it is tried first for
    /// bonds that carry YTM params.  Market providers that also support bonds
    /// will have lower priority and act as fallbacks.
    fn priority(&self) -> u8 {
        2
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            instrument_kinds: &[InstrumentKind::Bond],
            coverage: Coverage::default(),
            supports_latest: true,
            supports_historical: true,
            supports_search: false,
            supports_profile: false,
        }
    }

    fn rate_limit(&self) -> RateLimit {
        RateLimit {
            requests_per_minute: 10_000, // CPU-only, no network
            max_concurrency: 64,
            min_delay: std::time::Duration::ZERO,
        }
    }

    async fn get_latest_quote(
        &self,
        context: &QuoteContext,
        _instrument: ProviderInstrument,
    ) -> Result<Quote, MarketDataError> {
        let ytm_params = context.bond_ytm_params.as_ref().ok_or_else(|| {
            MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "bond_ytm_params missing in QuoteContext".to_string(),
            }
        })?;

        let bond = context.bond_metadata.as_ref().ok_or_else(|| {
            MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "bond_metadata missing in QuoteContext".to_string(),
            }
        })?;

        let today = Utc::now().date_naive();
        let settlement = today + chrono::Duration::days(ytm_params.settlement_days as i64);

        let price = price_bond(bond.coupon_rate, bond.face_value, bond.maturity_date,
            &ytm_params.ytm_source, settlement, &ytm_params.coupon_schedule,
            ytm_params.day_count, ytm_params.ex_coupon_days, ytm_params.pricing_method)?;

        let currency = context
            .currency_hint
            .as_deref()
            .unwrap_or("USD")
            .to_string();

        debug!(
            "INTERNAL_BOND_MODEL: asset={:?} price={} settlement={}",
            context.instrument, price, settlement
        );

        Ok(Quote::new(Utc::now(), price, currency, PROVIDER_ID.to_string()))
    }

    async fn get_historical_quotes(
        &self,
        context: &QuoteContext,
        _instrument: ProviderInstrument,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Quote>, MarketDataError> {
        let ytm_params = context.bond_ytm_params.as_ref().ok_or_else(|| {
            MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "bond_ytm_params missing in QuoteContext".to_string(),
            }
        })?;

        let bond = context.bond_metadata.as_ref().ok_or_else(|| {
            MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "bond_metadata missing in QuoteContext".to_string(),
            }
        })?;

        let currency = context
            .currency_hint
            .as_deref()
            .unwrap_or("USD")
            .to_string();

        let mut quotes = Vec::new();
        let mut date = start.date_naive();
        let end_date = end.date_naive();

        while date <= end_date {
            // Skip weekends (simple heuristic; bonds may trade on some holidays)
            use chrono::Datelike;
            let weekday = date.weekday();
            if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
                date += chrono::Duration::days(1);
                continue;
            }

            let settlement = date + chrono::Duration::days(ytm_params.settlement_days as i64);

            match price_bond(
                bond.coupon_rate,
                bond.face_value,
                bond.maturity_date,
                &ytm_params.ytm_source,
                settlement,
                &ytm_params.coupon_schedule,
                ytm_params.day_count,
                ytm_params.ex_coupon_days,
                ytm_params.pricing_method,
            ) {
                Ok(price) => {
                    let ts = date
                        .and_hms_opt(16, 0, 0)
                        .map(|dt| Utc.from_utc_datetime(&dt))
                        .unwrap_or_else(Utc::now);
                    quotes.push(Quote::new(ts, price, currency.clone(), PROVIDER_ID.to_string()));
                }
                Err(e) => {
                    warn!("INTERNAL_BOND_MODEL: skipping {} — {}", date, e);
                }
            }

            date += chrono::Duration::days(1);
        }

        Ok(quotes)
    }
}

// ---------------------------------------------------------------------------
// Shared pricing helper
// ---------------------------------------------------------------------------

/// Compute bond price (dirty or clean) for a given settlement date.
fn price_bond(
    coupon_rate: Decimal,
    face_value: Decimal,
    maturity_date: NaiveDate,
    ytm_source: &YtmSource,
    settlement: NaiveDate,
    coupon_schedule: &[NaiveDate],
    day_count: DayCountConvention,
    ex_coupon_days: i32,
    pricing_method: PricingMethod,
) -> Result<Decimal, MarketDataError> {
    use rust_decimal::prelude::ToPrimitive;
    let coupon_f = coupon_rate.to_f64().unwrap_or(0.0);
    let face_f = face_value.to_f64().unwrap_or(1000.0);

    let ytm_f = match ytm_source {
        YtmSource::Fixed(y) => y.to_f64().ok_or_else(|| MarketDataError::ProviderError {
            provider: PROVIDER_ID.to_string(),
            message: "YTM conversion failed".to_string(),
        })?,
        YtmSource::CurvePlusSpread { .. } => {
            return Err(MarketDataError::ProviderError {
                provider: PROVIDER_ID.to_string(),
                message: "CURVE_PLUS_SPREAD requires an external yield curve — not implemented yet".to_string(),
            });
        }
    };

    let clean = calculate_clean_price(
        ytm_f,
        settlement,
        maturity_date,
        coupon_f,
        face_f,
        coupon_schedule,
        day_count,
    )?;

    let price_f = match pricing_method {
        PricingMethod::Clean => clean,
        PricingMethod::Dirty => {
            let accrued = calculate_accrued_interest(
                coupon_f,
                face_f,
                settlement,
                coupon_schedule,
                day_count,
                ex_coupon_days,
            );
            clean + accrued
        }
    };

    Decimal::try_from(price_f).map_err(|_| MarketDataError::ProviderError {
        provider: PROVIDER_ID.to_string(),
        message: "Price f64→Decimal conversion failed".to_string(),
    })
}

// Need chrono::TimeZone for Utc.from_utc_datetime
use chrono::TimeZone;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// 3.25% semi-annual coupon, 3.50% YTM, ~1 year to maturity.
    /// Clean price should be slightly below par (ytm > coupon).
    #[test]
    fn test_clean_price_below_par_when_ytm_above_coupon() {
        let schedule = vec![d(2026, 6, 1), d(2026, 12, 1), d(2027, 6, 1)];
        let settlement = d(2026, 5, 1);
        let maturity = d(2027, 6, 1);
        let price = calculate_clean_price(
            0.035,
            settlement,
            maturity,
            0.0325,
            1000.0,
            &schedule,
            DayCountConvention::ActAct,
        )
        .unwrap();
        assert!(price < 1000.0, "price={}", price);
        assert!(price > 950.0, "price={}", price);
    }

    /// Bond at or after maturity returns face value.
    #[test]
    fn test_matured_bond_returns_face_value() {
        let schedule = vec![d(2025, 6, 1)];
        let settlement = d(2025, 7, 1); // after maturity
        let maturity = d(2025, 6, 1);
        let price = calculate_clean_price(
            0.035,
            settlement,
            maturity,
            0.0325,
            1000.0,
            &schedule,
            DayCountConvention::ActAct,
        )
        .unwrap();
        assert_eq!(price, 1000.0);
    }

    /// Accrued interest is zero when we are in the ex-coupon window.
    #[test]
    fn test_accrued_zero_in_ex_coupon_window() {
        let schedule = vec![d(2026, 1, 1), d(2026, 7, 1), d(2027, 1, 1)];
        // 5 days before the coupon → inside ex-coupon window of 7 days
        let settlement = d(2026, 6, 26);
        let accrued = calculate_accrued_interest(
            0.05,
            1000.0,
            settlement,
            &schedule,
            DayCountConvention::ActAct,
            7,
        );
        assert_eq!(accrued, 0.0);
    }

    /// Positive accrued interest outside ex-coupon window.
    #[test]
    fn test_accrued_positive_outside_ex_coupon_window() {
        let schedule = vec![d(2026, 1, 1), d(2026, 7, 1), d(2027, 1, 1)];
        let settlement = d(2026, 4, 1); // mid-period, far from ex-coupon
        let accrued = calculate_accrued_interest(
            0.05,
            1000.0,
            settlement,
            &schedule,
            DayCountConvention::ActAct,
            7,
        );
        assert!(accrued > 0.0, "accrued={}", accrued);
        // ~3 months into 6-month period → ~12.5 per $1000 face
        assert!(accrued > 10.0 && accrued < 20.0, "accrued={}", accrued);
    }
}
