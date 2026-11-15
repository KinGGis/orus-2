//! Comparison between a broker's own valuation and the one we compute.
//!
//! The account cards show the custodian's figure, so the computed total needs a
//! place where it stays visible: an unexplained gap between what the broker
//! reports and what our activity history reconstructs is precisely what a
//! track-record audit looks for.
//!
//! The arithmetic lives here rather than in the HTTP handler because a missing
//! side must never be silently read as zero — which would turn "unknown" into a
//! gap equal to the whole portfolio — and that rule deserves tests.

use rust_decimal::Decimal;

/// How a broker's valuation compares with ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ValuationGap {
    /// Broker figure minus ours; positive means the broker values us higher.
    pub difference: Option<Decimal>,
    /// The same gap relative to our own figure, as a percentage.
    pub difference_percent: Option<Decimal>,
}

/// Compares the two valuations, leaving the gap unknown when a side is missing.
///
/// The percentage is expressed against our computed figure, since that is the
/// one being checked. It is omitted when our figure is zero: a gap against
/// nothing has no meaningful ratio, and dividing would panic or yield infinity.
pub fn compare_valuations(broker: Option<Decimal>, local: Option<Decimal>) -> ValuationGap {
    let difference = match (broker, local) {
        (Some(broker_value), Some(local_value)) => Some(broker_value - local_value),
        _ => None,
    };

    let difference_percent = match (difference, local) {
        (Some(diff), Some(local_value)) if !local_value.is_zero() => {
            Some((diff / local_value) * Decimal::ONE_HUNDRED)
        }
        _ => None,
    };

    ValuationGap {
        difference,
        difference_percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn reports_the_gap_and_its_ratio_when_both_sides_are_known() {
        let gap = compare_valuations(Some(dec!(1100)), Some(dec!(1000)));

        assert_eq!(gap.difference, Some(dec!(100)));
        assert_eq!(gap.difference_percent, Some(dec!(10)));
    }

    #[test]
    fn signs_the_gap_against_our_own_figure() {
        let gap = compare_valuations(Some(dec!(900)), Some(dec!(1000)));

        assert_eq!(gap.difference, Some(dec!(-100)));
        assert_eq!(gap.difference_percent, Some(dec!(-10)));
    }

    #[test]
    fn leaves_the_gap_unknown_when_the_broker_figure_is_missing() {
        let gap = compare_valuations(None, Some(dec!(1000)));

        assert_eq!(gap.difference, None);
        assert_eq!(gap.difference_percent, None);
    }

    #[test]
    fn leaves_the_gap_unknown_when_our_figure_is_missing() {
        let gap = compare_valuations(Some(dec!(1000)), None);

        assert_eq!(gap.difference, None);
        assert_eq!(gap.difference_percent, None);
    }

    /// A zero computed total is a real state — an account whose activities are
    /// not imported yet. The absolute gap still holds; only the ratio cannot.
    #[test]
    fn omits_the_ratio_rather_than_dividing_by_zero() {
        let gap = compare_valuations(Some(dec!(1000)), Some(Decimal::ZERO));

        assert_eq!(gap.difference, Some(dec!(1000)));
        assert_eq!(gap.difference_percent, None);
    }

    #[test]
    fn reports_a_matching_pair_as_no_gap_rather_than_as_unknown() {
        let gap = compare_valuations(Some(dec!(1000)), Some(dec!(1000)));

        assert_eq!(gap.difference, Some(Decimal::ZERO));
        assert_eq!(gap.difference_percent, Some(Decimal::ZERO));
    }
}
