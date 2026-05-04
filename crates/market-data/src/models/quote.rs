use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::instrument::InstrumentId;
use super::provider_params::ProviderOverrides;
use super::types::{Currency, ProviderId};

// =============================================================================
// Internal YTM bond pricing types
// =============================================================================

/// Source of the yield-to-maturity used for internal bond pricing.
#[derive(Clone, Debug)]
pub enum YtmSource {
    /// Fixed YTM entered by the user (e.g. 0.0325 = 3.25 %)
    Fixed(Decimal),
    /// Market yield curve + a constant z-spread in basis points
    CurvePlusSpread { spread_bps: Decimal },
}

/// Day-count convention for accrued-interest and year-fraction calculations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayCountConvention {
    ActAct,
    Act365,
    Act360,
    Thirty360,
}

impl DayCountConvention {
    /// Parse from a string representation (e.g. "ACT/ACT", "30/360").
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ACT/ACT" => Some(Self::ActAct),
            "ACT/365" => Some(Self::Act365),
            "ACT/360" => Some(Self::Act360),
            "30/360" => Some(Self::Thirty360),
            _ => None,
        }
    }

    /// Fraction of a year between two dates using this convention.
    pub fn year_fraction(self, from: NaiveDate, to: NaiveDate) -> f64 {
        let days = (to - from).num_days() as f64;
        match self {
            Self::ActAct => days / 365.25,
            Self::Act365 => days / 365.0,
            Self::Act360 => days / 360.0,
            Self::Thirty360 => days / 360.0,
        }
    }
}

/// Whether the calculated price includes accrued interest (dirty) or not (clean).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PricingMethod {
    /// Dirty price: clean price + accrued interest
    Dirty,
    /// Clean price only
    Clean,
}

impl PricingMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "DIRTY" => Some(Self::Dirty),
            "CLEAN" => Some(Self::Clean),
            _ => None,
        }
    }
}

/// All parameters needed to compute a daily bond price internally.
#[derive(Clone, Debug)]
pub struct BondYtmParams {
    pub ytm_source: YtmSource,
    pub day_count: DayCountConvention,
    /// Settlement lag in calendar days (typically 1 for govt bonds)
    pub settlement_days: i32,
    /// Ex-coupon window in days before coupon date where accrued interest is zeroed
    pub ex_coupon_days: i32,
    pub pricing_method: PricingMethod,
    /// Sorted coupon payment dates
    pub coupon_schedule: Vec<NaiveDate>,
}

// =============================================================================
// Bond quote metadata (for market-provider routing)
// =============================================================================

/// Bond metadata needed for yield-curve-based price calculation.
#[derive(Clone, Debug)]
pub struct BondQuoteMetadata {
    /// Annual coupon rate as a decimal (0.05 = 5%)
    pub coupon_rate: Decimal,
    /// Maturity date of the bond
    pub maturity_date: NaiveDate,
    /// Face/par value of the bond
    pub face_value: Decimal,
    /// Coupon payment frequency: "SEMI_ANNUAL", "ANNUAL", "QUARTERLY", "ZERO"
    pub coupon_frequency: String,
}

/// Request context for quote fetching
#[derive(Clone, Debug)]
pub struct QuoteContext {
    /// Canonical instrument
    pub instrument: InstrumentId,

    /// Pre-resolved provider overrides (from Asset.provider_overrides)
    pub overrides: Option<ProviderOverrides>,

    /// Currency hint
    pub currency_hint: Option<Currency>,

    /// Preferred provider (from Asset.preferred_provider)
    pub preferred_provider: Option<ProviderId>,

    /// Bond metadata for yield-curve-based pricing (coupon, maturity, face value)
    pub bond_metadata: Option<BondQuoteMetadata>,

    /// YTM parameters for internal daily dirty-price calculation.
    /// Present only when asset.quote_mode == INTERNAL_YTM.
    pub bond_ytm_params: Option<BondYtmParams>,
}

/// Market data quote
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    /// Timestamp of the quote
    pub timestamp: DateTime<Utc>,

    /// Opening price (optional for intraday)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<Decimal>,

    /// High price (optional for intraday)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<Decimal>,

    /// Low price (optional for intraday)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<Decimal>,

    /// Closing/current price (required)
    pub close: Decimal,

    /// Trading volume (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<Decimal>,

    /// Quote currency
    pub currency: String,

    /// Source of the quote (MANUAL, YAHOO, ALPHA_VANTAGE, etc.)
    pub source: String,
}

impl Quote {
    /// Create a new quote with minimal required fields
    pub fn new(timestamp: DateTime<Utc>, close: Decimal, currency: String, source: String) -> Self {
        Self {
            timestamp,
            open: None,
            high: None,
            low: None,
            close,
            volume: None,
            currency,
            source,
        }
    }

    /// Create a full OHLCV quote
    #[allow(clippy::too_many_arguments)]
    pub fn ohlcv(
        timestamp: DateTime<Utc>,
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        volume: Decimal,
        currency: String,
        source: String,
    ) -> Self {
        Self {
            timestamp,
            open: Some(open),
            high: Some(high),
            low: Some(low),
            close,
            volume: Some(volume),
            currency,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_quote_new() {
        let quote = Quote::new(
            Utc::now(),
            dec!(150.25),
            "USD".to_string(),
            "YAHOO".to_string(),
        );
        assert_eq!(quote.close, dec!(150.25));
        assert_eq!(quote.currency, "USD");
        assert!(quote.open.is_none());
    }

    #[test]
    fn test_quote_ohlcv() {
        let quote = Quote::ohlcv(
            Utc::now(),
            dec!(148.00),
            dec!(152.00),
            dec!(147.50),
            dec!(150.25),
            dec!(1000000),
            "USD".to_string(),
            "YAHOO".to_string(),
        );
        assert_eq!(quote.open, Some(dec!(148.00)));
        assert_eq!(quote.high, Some(dec!(152.00)));
        assert_eq!(quote.low, Some(dec!(147.50)));
        assert_eq!(quote.close, dec!(150.25));
        assert_eq!(quote.volume, Some(dec!(1000000)));
    }
}
