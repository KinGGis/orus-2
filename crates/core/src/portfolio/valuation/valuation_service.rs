use crate::errors::{CalculatorError, Error as CoreError, Result as CoreResult};
use crate::fx::currency::normalize_currency_code;
use crate::fx::FxServiceTrait;
use crate::portfolio::snapshot::SnapshotServiceTrait;
use crate::portfolio::valuation::valuation_calculator::calculate_valuation;
use crate::portfolio::valuation::valuation_model::{DailyAccountValuation, ValuationSource};
use crate::portfolio::valuation::ValuationRepositoryTrait;
use crate::quotes::{DataSource, QuoteServiceTrait};
use crate::utils::time_utils;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, Weekday};
use rust_decimal::Decimal;
use log::{debug, error, warn};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::DailyFxRateMap;
use crate::accounts::AccountRepositoryTrait;
use crate::constants::PORTFOLIO_TOTAL_ACCOUNT_ID;

/// Longest run of calendar days a reported NAV is carried over (a Thursday
/// close through an Easter Monday).
const MAX_BROKER_NAV_CARRY_DAYS: i64 = 4;

/// The broker NAV that applies on `date`.
///
/// Brokers report on trading days only. Falling back to the computed figure in
/// between made every weekend drop to whatever local quotes could price, which
/// showed up as weekly crashes in the return series. The market is closed on
/// those days, so the last reported NAV still holds — the same convention the
/// broker applies itself.
///
/// A day is filled only when it is a weekend or lies between two reported days
/// (a holiday). A trailing weekday is left computed: carried rows are persisted
/// as reported, and filling it would let a stale NAV extend itself on every
/// recalculation until the next sync.
fn broker_nav_on(navs: &BTreeMap<NaiveDate, Decimal>, date: NaiveDate) -> Option<Decimal> {
    if let Some(nav) = navs.get(&date) {
        return Some(*nav);
    }
    let (previous_date, previous_nav) = navs.range(..date).next_back()?;
    if (date - *previous_date).num_days() > MAX_BROKER_NAV_CARRY_DAYS {
        return None;
    }
    let weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
    let between_reports = navs.range(date..).next().is_some();
    (weekend || between_reports).then_some(*previous_nav)
}

/// Sum the portfolios' own valuations into TOTAL on the days any of them is
/// reported rather than computed.
///
/// Only the value is replaced. Net contribution stays as computed for TOTAL
/// from the activities of every portfolio, in base currency at each flow's own
/// rate, so returns keep treating deposits as flows. Days with no reported
/// member are left untouched, preserving the computed series exactly. The rows
/// stay `Calculated`: they are derived, and marking them reported would make
/// the next recalculation replay them as if a broker had sent them.
pub(crate) fn consolidate_with_members(
    totals: &mut [DailyAccountValuation],
    members: &[DailyAccountValuation],
) {
    let mut by_date: HashMap<NaiveDate, (Decimal, bool)> = HashMap::new();
    for member in members {
        let entry = by_date
            .entry(member.valuation_date)
            .or_insert((Decimal::ZERO, false));
        entry.0 += member.total_value * member.fx_rate_to_base;
        entry.1 |= member.source == ValuationSource::BrokerImported;
    }

    for total in totals.iter_mut() {
        if let Some((value, true)) = by_date.get(&total.valuation_date).copied() {
            total.total_value = value;
            total.investment_market_value = value - total.cash_balance;
        }
    }
}

/// Controls the scope of a valuation history recalculation.
#[derive(Clone, Debug)]
pub enum ValuationRecalcMode {
    /// Delete all valuations and recalculate from the first snapshot.
    Full,
    /// Resume from the latest saved valuation date, only computing new dates forward.
    IncrementalFromLast,
    /// Delete valuations from `date` forward and recalculate from that date.
    SinceDate(NaiveDate),
}

#[async_trait]
pub trait ValuationServiceTrait: Send + Sync {
    /// Ensures the valuation history for the account is calculated and stored.
    ///
    /// The `mode` controls how much history is recomputed:
    /// - `Full`: delete all valuations and recalculate from the first snapshot.
    /// - `IncrementalFromLast`: resume from the latest saved valuation date.
    /// - `SinceDate(date)`: delete valuations from `date` forward and recalculate from that date.
    ///
    /// Args:
    ///     account_id: The ID of the account ("TOTAL" for portfolio aggregate).
    ///     mode: Controls the recalculation scope.
    async fn calculate_valuation_history(
        &self,
        account_id: &str,
        mode: ValuationRecalcMode,
    ) -> CoreResult<()>;

    /// Loads the valuation data for the account within the specified date range.
    ///
    /// Args:
    ///     account_id: The ID of the account ("TOTAL" for portfolio aggregate).
    ///     start_date_opt: Optional start date (inclusive).
    ///     end_date_opt: Optional end date (inclusive).
    ///
    /// Returns:
    ///     A `Result` containing a vector of `DailyAccountValuation` or an error.
    fn get_historical_valuations(
        &self,
        account_id: &str,
        start_date_opt: Option<NaiveDate>,
        end_date_opt: Option<NaiveDate>,
    ) -> CoreResult<Vec<DailyAccountValuation>>;

    /// Loads the latest valuation history record for a list of accounts.
    ///
    /// Args:
    ///     account_ids: A slice of account IDs.
    ///
    /// Returns:
    ///     A `Result` containing a `HashMap` mapping account IDs to their
    ///     latest `DailyAccountValuation` (if found), or `None` if no history exists.
    ///     latest `DailyAccountValuation` for each account that has one.
    fn get_latest_valuations(
        &self,
        account_ids: &[String],
    ) -> CoreResult<Vec<DailyAccountValuation>>;

    fn get_valuations_on_date(
        &self,
        account_ids: &[String],
        date: NaiveDate,
    ) -> CoreResult<Vec<DailyAccountValuation>>;
}

#[derive(Clone)]
pub struct ValuationService {
    base_currency: Arc<RwLock<String>>,
    valuation_repository: Arc<dyn ValuationRepositoryTrait>,
    snapshot_service: Arc<dyn SnapshotServiceTrait>,
    quote_service: Arc<dyn QuoteServiceTrait>,
    fx_service: Arc<dyn FxServiceTrait>,
    account_repository: Arc<dyn AccountRepositoryTrait>,
}

impl ValuationService {
    pub fn new(
        base_currency: Arc<RwLock<String>>,
        valuation_repository: Arc<dyn ValuationRepositoryTrait>,
        snapshot_service: Arc<dyn SnapshotServiceTrait>,
        quote_service: Arc<dyn QuoteServiceTrait>,
        fx_service: Arc<dyn FxServiceTrait>,
        account_repository: Arc<dyn AccountRepositoryTrait>,
    ) -> Self {
        Self {
            base_currency,
            snapshot_service,
            quote_service,
            fx_service,
            valuation_repository,
            account_repository,
        }
    }

    /// Replace the TOTAL value with the sum of its portfolios on every day at
    /// least one of them carries a reported valuation.
    ///
    /// TOTAL is otherwise valued from its own aggregated holdings, which never
    /// sees the figures a broker or fund administrator reports for a single
    /// portfolio, so the fund total kept showing the locally computed value of a
    /// portfolio whose own page showed the reported one.
    fn consolidate_reported_values(&self, totals: &mut [DailyAccountValuation]) -> CoreResult<()> {
        let (Some(first), Some(last)) = (
            totals.iter().map(|v| v.valuation_date).min(),
            totals.iter().map(|v| v.valuation_date).max(),
        ) else {
            return Ok(());
        };

        let mut members = Vec::new();
        for account in self.account_repository.list(None, Some(false), None)? {
            if account.id == PORTFOLIO_TOTAL_ACCOUNT_ID {
                continue;
            }
            members.extend(self.valuation_repository.get_historical_valuations(
                &account.id,
                Some(first),
                Some(last),
            )?);
        }

        consolidate_with_members(totals, &members);
        Ok(())
    }

    async fn fetch_fx_rates_for_range(
        &self,
        pairs: &HashSet<(String, String)>,
        start_date: NaiveDate,
        end_date: NaiveDate,
        base_currency: &str,
    ) -> CoreResult<HashMap<NaiveDate, DailyFxRateMap>> {
        if pairs.is_empty() {
            return Ok(HashMap::new());
        }

        let mut fx_rates_by_date: HashMap<NaiveDate, DailyFxRateMap> = HashMap::new();
        let date_range = time_utils::get_days_between(start_date, end_date);

        for current_date in date_range {
            let mut daily_map: DailyFxRateMap = HashMap::with_capacity(pairs.len());
            for (from_curr, to_curr) in pairs {
                match self
                    .fx_service
                    .get_exchange_rate_for_date(from_curr, to_curr, current_date)
                {
                    Ok(rate) => {
                        daily_map.insert((from_curr.clone(), to_curr.clone()), rate);
                    }
                    Err(e) => {
                        let via_base = if from_curr != base_currency && to_curr != base_currency {
                            let from_to_base = self.fx_service.get_exchange_rate_for_date(
                                from_curr,
                                base_currency,
                                current_date,
                            );
                            let to_to_base = self.fx_service.get_exchange_rate_for_date(
                                to_curr,
                                base_currency,
                                current_date,
                            );

                            match (from_to_base, to_to_base) {
                                (Ok(from_base_rate), Ok(to_base_rate))
                                    if !to_base_rate.is_zero() =>
                                {
                                    Some(from_base_rate / to_base_rate)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };

                        if let Some(rate) = via_base {
                            debug!(
                                "Resolved FX rate {}->{} for {} via base currency {}.",
                                from_curr, to_curr, current_date, base_currency
                            );
                            daily_map.insert((from_curr.clone(), to_curr.clone()), rate);
                        } else {
                            warn!(
                                "Failed to get FX rate {}->{} for date {}: {}. Valuation for this date might be affected.",
                                from_curr, to_curr, current_date, e
                            );
                        }
                    }
                }
            }
            if !daily_map.is_empty() {
                fx_rates_by_date.insert(current_date, daily_map);
            }
        }

        Ok(fx_rates_by_date)
    }
}

#[async_trait]
impl ValuationServiceTrait for ValuationService {
    async fn calculate_valuation_history(
        &self,
        account_id: &str,
        mode: ValuationRecalcMode,
    ) -> CoreResult<()> {
        let total_start_time = Instant::now();
        debug!(
            "Starting valuation data update/recalculation for account '{}', mode: {:?}",
            account_id, mode
        );

        let mut calculation_start_date: Option<NaiveDate> = None;

        // Read before any deletion: a full recalculation wipes every row of the
        // account, and these rows are the only copy of the figures the broker
        // reported. Reading them afterwards silently dropped them all.
        let broker_navs: BTreeMap<NaiveDate, Decimal> = self
            .valuation_repository
            .get_historical_valuations(account_id, None, None)?
            .into_iter()
            .filter(|v| v.source == ValuationSource::BrokerImported)
            .map(|v| (v.valuation_date, v.total_value))
            .collect();

        match &mode {
            ValuationRecalcMode::Full => {
                self.valuation_repository
                    .delete_valuations_for_account(account_id, None)
                    .await?;
            }
            ValuationRecalcMode::SinceDate(date) => {
                self.valuation_repository
                    .delete_valuations_for_account(account_id, Some(*date))
                    .await?;
                calculation_start_date = Some(*date);
            }
            ValuationRecalcMode::IncrementalFromLast => {
                let last_saved_date_opt = self
                    .valuation_repository
                    .load_latest_valuation_date(account_id)?;

                if let Some(last_saved) = last_saved_date_opt {
                    calculation_start_date = Some(last_saved);
                }
            }
        }

        let snapshots_to_process = self
            .snapshot_service
            .get_daily_holdings_snapshots(account_id, calculation_start_date, None)
            .map_err(|e| {
                CoreError::Calculation(CalculatorError::Calculation(format!(
                    "Failed snapshot fetch for account {}: {}",
                    account_id, e
                )))
            })?;

        if snapshots_to_process.is_empty() {
            return Ok(());
        }

        let actual_calculation_start_date = snapshots_to_process.first().unwrap().snapshot_date;
        let calculation_end_date = snapshots_to_process.last().unwrap().snapshot_date;

        let mut required_asset_ids = HashSet::new();
        let mut required_fx_pairs = HashSet::new();
        let base_curr = {
            let base_guard = self.base_currency.read().unwrap();
            normalize_currency_code(&base_guard).to_string()
        };
        let mut normalized_account_currency: Option<String> = None;

        for snapshot in &snapshots_to_process {
            let account_curr = normalize_currency_code(&snapshot.currency);
            if normalized_account_currency.is_none() {
                normalized_account_currency = Some(account_curr.to_string());
            }
            if account_curr != base_curr {
                required_fx_pairs.insert((account_curr.to_string(), base_curr.clone()));
            }
            for (asset_id, position) in &snapshot.positions {
                required_asset_ids.insert(asset_id.clone());
                let position_currency = normalize_currency_code(&position.currency);
                if position_currency != account_curr {
                    required_fx_pairs
                        .insert((position_currency.to_string(), account_curr.to_string()));
                }
            }
            for cash_curr in snapshot.cash_balances.keys() {
                let normalized_cash_currency = normalize_currency_code(cash_curr);
                if normalized_cash_currency != account_curr {
                    required_fx_pairs.insert((
                        normalized_cash_currency.to_string(),
                        account_curr.to_string(),
                    ));
                }
            }
        }

        let account_curr = normalized_account_currency.unwrap_or_else(|| base_curr.clone());

        // Fetch quotes with single call
        let quotes_vec = self.quote_service.get_quotes_in_range_filled(
            &required_asset_ids,
            actual_calculation_start_date,
            calculation_end_date,
        )?;

        for quote in &quotes_vec {
            let normalized_quote_currency = normalize_currency_code(&quote.currency);
            if normalized_quote_currency != account_curr.as_str() {
                required_fx_pairs
                    .insert((normalized_quote_currency.to_string(), account_curr.clone()));
            }
        }

        let fx_rates_by_date = self
            .fetch_fx_rates_for_range(
                &required_fx_pairs,
                actual_calculation_start_date,
                calculation_end_date,
                &base_curr,
            )
            .await?;

        // Build quotes_by_date and track which assets have ANY quotes at all.
        //
        // Only provider quotes count towards `assets_with_quotes`. A broker or
        // manually marked price covers a single day by design, so counting it
        // here would make every other day look like a provider data gap and
        // silently drop the whole day from the valuation history below.
        let mut assets_with_quotes: HashSet<String> = HashSet::new();
        let quotes_by_date = {
            let mut map = HashMap::new();
            for quote in quotes_vec {
                if quote.data_source != DataSource::Manual {
                    assets_with_quotes.insert(quote.asset_id.clone());
                }
                map.entry(quote.timestamp.date_naive())
                    .or_insert_with(HashMap::new)
                    .insert(quote.asset_id.clone(), quote);
            }
            map
        };

        let newly_calculated_valuations: Vec<DailyAccountValuation> = snapshots_to_process
            .into_iter()
            .filter_map(|holdings_snapshot| {
                let current_date = holdings_snapshot.snapshot_date;
                let account_id_clone = account_id.to_string();
                let base_curr_clone = base_curr.clone();

                let quotes_for_current_date =
                    quotes_by_date.get(&current_date).cloned().unwrap_or_default();

                let fx_for_current_date = fx_rates_by_date
                    .get(&current_date)
                    .cloned()
                    .unwrap_or_default();

                // Check for assets missing quotes that SHOULD have quotes
                // (i.e., they have quotes elsewhere, just not for this date - indicates a gap)
                // Assets with NO quotes at all are skipped - they'll be valued at ZERO
                // and the health check will detect them
                let missing_quotes_with_gap: Vec<_> = holdings_snapshot
                    .positions
                    .iter()
                    .filter(|(_, position)| !position.quantity.is_zero())
                    .map(|(symbol, _)| symbol)
                    // Only flag as missing if the asset HAS quotes (somewhere) but not for this date
                    .filter(|symbol| assets_with_quotes.contains(*symbol))
                    .filter(|symbol| !quotes_for_current_date.contains_key(*symbol))
                    .cloned()
                    .collect();

                if !missing_quotes_with_gap.is_empty() {
                    debug!(
                        "Quote gap for {:?} on {} (account '{}'). Skipping day.",
                        missing_quotes_with_gap, current_date, account_id_clone
                    );
                    return None;
                }

                // Check if there are any positions that need quotes
                // but only consider positions that HAVE quotes somewhere
                let has_quotable_positions = holdings_snapshot
                    .positions
                    .keys()
                    .any(|symbol| assets_with_quotes.contains(symbol));

                if quotes_for_current_date.is_empty() && has_quotable_positions {
                    debug!(
                        "No quotes for date {} (account '{}'). Skipping day.",
                        current_date, account_id_clone
                    );
                    return None;
                }
                let normalized_account_curr =
                    normalize_currency_code(&holdings_snapshot.currency).to_string();
                if normalized_account_curr != base_curr_clone
                    && !fx_for_current_date
                        .contains_key(&(normalized_account_curr.clone(), base_curr_clone.clone()))
                {
                    warn!(
                        "Base currency FX rate ({}->{}) missing for {} (account '{}'). Skipping day.",
                        normalized_account_curr, base_curr_clone, current_date, account_id_clone
                    );
                    return None;
                }

                match calculate_valuation(
                    &holdings_snapshot,
                    &quotes_for_current_date,
                    &fx_for_current_date,
                    current_date,
                    &base_curr_clone,
                ) {
                    Ok(valuation_result) => Some(valuation_result),
                    Err(e) => {
                        error!(
                            "Failed to calculate valuation for account {} on date {}: {}. Skipping this date.",
                            account_id, current_date, e
                        );
                        None
                    }
                }
            })
            .collect();

        if !newly_calculated_valuations.is_empty() {
            // IBKR-style brokers report their own net asset value. Only the
            // market value depends on quotes we may be missing, so the broker
            // NAV replaces that figure while cash, cost basis and net
            // contribution stay as computed from activities — those are
            // derived from the transaction history and remain trustworthy.
            let mut valuations_to_save: Vec<DailyAccountValuation> = newly_calculated_valuations
                .into_iter()
                .map(|mut valuation| {
                    if let Some(nav) = broker_nav_on(&broker_navs, valuation.valuation_date) {
                        valuation.investment_market_value = nav - valuation.cash_balance;
                        valuation.total_value = nav;
                        valuation.source = ValuationSource::BrokerImported;
                    }
                    valuation
                })
                .collect();

            if account_id == PORTFOLIO_TOTAL_ACCOUNT_ID {
                self.consolidate_reported_values(&mut valuations_to_save)?;
            }

            self.valuation_repository
                .save_valuations(&valuations_to_save)
                .await?;
        }

        let total_duration = total_start_time.elapsed();
        debug!(
            "Successfully updated/recalculated valuation data for account '{}' in {:?}",
            account_id, total_duration
        );

        Ok(())
    }

    fn get_historical_valuations(
        &self,
        account_id: &str,
        start_date_opt: Option<NaiveDate>,
        end_date_opt: Option<NaiveDate>,
    ) -> CoreResult<Vec<DailyAccountValuation>> {
        debug!(
            "Loading historical valuations for account '{}' from {:?} to {:?}",
            account_id, start_date_opt, end_date_opt
        );
        self.valuation_repository.get_historical_valuations(
            account_id,
            start_date_opt,
            end_date_opt,
        )
    }

    fn get_latest_valuations(
        &self,
        account_ids: &[String],
    ) -> CoreResult<Vec<DailyAccountValuation>> {
        debug!("Loading latest valuations for accounts: {:?}", account_ids);
        self.valuation_repository.get_latest_valuations(account_ids)
    }

    fn get_valuations_on_date(
        &self,
        account_ids: &[String],
        date: NaiveDate,
    ) -> CoreResult<Vec<DailyAccountValuation>> {
        debug!(
            "Loading valuations for accounts {:?} on date {}",
            account_ids, date
        );
        self.valuation_repository
            .get_valuations_on_date(account_ids, date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn row(
        account_id: &str,
        day: u32,
        total: Decimal,
        fx: Decimal,
        source: ValuationSource,
    ) -> DailyAccountValuation {
        DailyAccountValuation {
            id: format!("{account_id}_{day}"),
            account_id: account_id.to_string(),
            valuation_date: NaiveDate::from_ymd_opt(2026, 11, day).unwrap(),
            account_currency: "GBP".to_string(),
            base_currency: "EUR".to_string(),
            fx_rate_to_base: fx,
            cash_balance: dec!(10),
            investment_market_value: total - dec!(10),
            total_value: total,
            cost_basis: Decimal::ZERO,
            net_contribution: dec!(500),
            calculated_at: chrono::Utc::now(),
            source,
        }
    }

    #[test]
    fn total_sums_members_in_base_currency_only_on_reported_days() {
        use ValuationSource::{BrokerImported, Calculated};

        let mut totals = vec![
            row("TOTAL", 1, dec!(900), Decimal::ONE, Calculated),
            row("TOTAL", 2, dec!(950), Decimal::ONE, Calculated),
        ];
        let members = vec![
            row("ibkr", 1, dec!(100), dec!(1.2), Calculated),
            row("pe", 1, dec!(300), Decimal::ONE, Calculated),
            row("ibkr", 2, dec!(200), dec!(1.2), BrokerImported),
            row("pe", 2, dec!(300), Decimal::ONE, Calculated),
        ];

        consolidate_with_members(&mut totals, &members);

        assert_eq!(totals[0].total_value, dec!(900));
        assert_eq!(totals[1].total_value, dec!(540));
        assert_eq!(totals[1].investment_market_value, dec!(530));
        assert_eq!(totals[1].net_contribution, dec!(500));
        assert_eq!(totals[1].source, Calculated);
    }

    #[test]
    fn broker_nav_is_carried_over_weekends_and_holidays_only() {
        let d = |day| NaiveDate::from_ymd_opt(2026, 8, day).unwrap();
        // Fri 21, Mon 24 missing (holiday), Tue 25, then nothing after Fri 28.
        let navs = BTreeMap::from([(d(21), dec!(100)), (d(25), dec!(110)), (d(28), dec!(120))]);

        assert_eq!(broker_nav_on(&navs, d(21)), Some(dec!(100)));
        assert_eq!(broker_nav_on(&navs, d(22)), Some(dec!(100)));
        assert_eq!(broker_nav_on(&navs, d(24)), Some(dec!(100)));
        assert_eq!(broker_nav_on(&navs, d(29)), Some(dec!(120)));
        assert_eq!(broker_nav_on(&navs, d(30)), Some(dec!(120)));
        // Trailing weekday after the last report stays computed.
        assert_eq!(broker_nav_on(&navs, d(31)), None);
        assert_eq!(broker_nav_on(&navs, d(20)), None);
    }
}
