//! [`BrokerApiClient`] implementation backed by the IBKR MCP server.
//!
//! The MCP consent is scoped to a single account and exposes no account
//! identifier, so the connection, brokerage and account are synthesised with
//! stable ids. Everything else is read straight from IBKR.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::json;
use tokio::sync::Mutex;

use wealthfolio_core::errors::{Error, Result};

use super::mcp::McpClient;
use super::models::{
    IbkrAccountSummary, IbkrBalancesResponse, IbkrPerformanceAccount, IbkrPerformanceResponse,
    IbkrPositionsResponse, IbkrTrade, IbkrTradesResponse, TRADE_PERIODS,
};
use super::oauth::IbkrTokenManager;
use crate::broker::{
    AccountOwner, AccountUniversalActivity, AccountUniversalActivityCurrency,
    AccountUniversalActivitySymbol, BrokerAccount, BrokerAccountBalance, BrokerApiClient,
    BrokerBalanceTotal, BrokerBrokerage, BrokerConnection, BrokerConnectionBrokerage,
    BrokerHoldingsResponse, HoldingsAccount, HoldingsBalance, HoldingsCurrency,
    HoldingsInnerSymbol, HoldingsPosition, HoldingsSymbol, PaginatedUniversalActivity,
    PaginationDetails,
};

/// Stable id of the synthetic connection representing the MCP grant.
pub const IBKR_CONNECTION_ID: &str = "ibkr-mcp";
/// Stable id of the synthetic account. The MCP consent covers exactly one
/// account and never discloses its number.
pub const IBKR_ACCOUNT_ID: &str = "IBKR-MCP-PRIMARY";
pub const IBKR_BROKERAGE_ID: &str = "ibkr-mcp-brokerage";
pub const IBKR_BROKERAGE_SLUG: &str = "INTERACTIVE_BROKERS";
pub const IBKR_SOURCE_SYSTEM: &str = "IBKR_MCP";

const DEFAULT_PAGE_LIMIT: i64 = 500;

/// Daily net asset value history reported by IBKR for the connected account.
#[derive(Debug, Clone, Default)]
pub struct IbkrNavHistory {
    /// Base currency the NAV figures are expressed in.
    pub currency: Option<String>,
    /// Ascending by date.
    pub points: Vec<(NaiveDate, Decimal)>,
}

/// Picks the widest usable NAV series across IBKR's performance windows.
///
/// The windows are nested (`1D` ⊂ `7D` ⊂ … ⊂ `1Y`), so the longest one
/// subsumes the others and is the only one worth importing. Entries whose
/// date or NAV cannot be read are dropped rather than failing the sync.
fn longest_nav_series(account: &IbkrPerformanceAccount) -> Vec<(NaiveDate, Decimal)> {
    let Some(periods) = account.periods.as_ref() else {
        return Vec::new();
    };

    let mut best: Vec<(NaiveDate, Decimal)> = Vec::new();
    for period in periods.values() {
        let (Some(dates), Some(navs)) = (period.dates.as_ref(), period.nav.as_ref()) else {
            continue;
        };

        let series: Vec<(NaiveDate, Decimal)> = dates
            .iter()
            .zip(navs.iter())
            .filter_map(|(raw_date, nav)| {
                let date = NaiveDate::parse_from_str(raw_date, "%Y%m%d").ok()?;
                let value = Decimal::from_f64_retain(*nav)?;
                Some((date, value.round_dp(NAV_DECIMAL_PRECISION)))
            })
            .collect();

        if series.len() > best.len() {
            best = series;
        }
    }

    best.sort_by_key(|(date, _)| *date);
    best.dedup_by_key(|(date, _)| *date);
    best
}

/// IBKR reports NAV with far more precision than is meaningful for money.
const NAV_DECIMAL_PRECISION: u32 = 8;

pub struct IbkrMcpClient {
    mcp: McpClient,
    /// Trades are fetched as five overlapping windows; caching the merged,
    /// deduplicated result keeps a paginated sync from refetching all of them
    /// on every page.
    trades_cache: Mutex<Option<Vec<AccountUniversalActivity>>>,
}

impl IbkrMcpClient {
    pub fn new(http: reqwest::Client, tokens: Arc<IbkrTokenManager>) -> Self {
        Self {
            mcp: McpClient::new(http, tokens),
            trades_cache: Mutex::new(None),
        }
    }

    /// Portfolio allocations straight from IBKR's own classification, used in
    /// place of the local taxonomy join for this account.
    pub async fn allocations(
        &self,
    ) -> Result<wealthfolio_core::portfolio::allocation::PortfolioAllocations> {
        super::allocation::fetch_allocations(&self.mcp).await
    }

    async fn account_summary(&self) -> Result<IbkrAccountSummary> {        let value = self.mcp.call_tool("get_account_summary", json!({})).await?;
        serde_json::from_value(value)
            .map_err(|e| Error::Unexpected(format!("Unexpected IBKR account summary shape: {e}")))
    }

    async fn base_currency(&self) -> Option<String> {
        let value = self
            .mcp
            .call_tool("get_pa_performance_all_periods", json!({}))
            .await
            .ok()?;
        let parsed: IbkrPerformanceResponse = serde_json::from_value(value).ok()?;
        parsed.accounts?.account?.base_currency
    }

    /// Daily net asset value history exactly as IBKR reports it.
    ///
    /// This is the custodian's own books rather than a figure rebuilt from
    /// quotes, so it stays correct for instruments our market-data providers
    /// cannot price.
    pub async fn nav_history(&self) -> Result<IbkrNavHistory> {
        let value = self
            .mcp
            .call_tool("get_pa_performance_all_periods", json!({}))
            .await?;
        let parsed: IbkrPerformanceResponse = serde_json::from_value(value)
            .map_err(|e| Error::Unexpected(format!("Unexpected IBKR performance shape: {e}")))?;

        let account = parsed
            .accounts
            .and_then(|a| a.account)
            .ok_or_else(|| Error::Unexpected("IBKR returned no performance account".into()))?;

        Ok(IbkrNavHistory {
            currency: account.base_currency.clone(),
            points: longest_nav_series(&account),
        })
    }

    /// Fetch every available trade window and merge them.
    ///
    /// The windows overlap (year-to-date covers the recent quarters), so
    /// deduplication by `trade_id` is mandatory — without it a sync would
    /// double-count roughly a third of the history.
    async fn load_trades(&self) -> Result<Vec<AccountUniversalActivity>> {
        if let Some(cached) = self.trades_cache.lock().await.as_ref() {
            return Ok(cached.clone());
        }

        let mut windows: Vec<(&str, std::result::Result<IbkrTradesResponse, String>)> = Vec::new();
        for period in TRADE_PERIODS {
            let fetched = self
                .mcp
                .call_tool("get_account_trades", json!({ "period": period }))
                .await
                .and_then(|value| {
                    serde_json::from_value::<IbkrTradesResponse>(value).map_err(|e| {
                        Error::Unexpected(format!(
                            "Unexpected IBKR trades shape for period {period}: {e}"
                        ))
                    })
                })
                .map_err(|e| e.to_string());
            windows.push((period, fetched));
        }

        let activities = merge_trade_windows(windows)?;

        *self.trades_cache.lock().await = Some(activities.clone());
        Ok(activities)
    }
}

/// Merges the overlapping trade windows into one deduplicated, dated list.
///
/// A window that fails is skipped rather than fatal: the windows overlap
/// heavily, so one bad window costs little history, whereas failing the whole
/// call drops every activity — and in HOLDINGS mode it does so as a mere
/// warning sitting behind a successful positions sync, leaving the user with
/// holdings but an empty activity list. Only a total blackout is an error.
fn merge_trade_windows(
    windows: Vec<(&str, std::result::Result<IbkrTradesResponse, String>)>,
) -> Result<Vec<AccountUniversalActivity>> {
    let total_windows = windows.len();
    let mut seen = std::collections::HashSet::new();
    let mut activities = Vec::new();
    let mut skipped_fx = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for (period, window) in windows {
        let parsed = match window {
            Ok(parsed) => parsed,
            Err(e) => {
                log::warn!("IBKR MCP: trade window {period} failed: {e}");
                failures.push(period.to_string());
                continue;
            }
        };

        for trade in parsed.trades {
            let Some(trade_id) = trade.trade_id.clone() else {
                continue;
            };
            if !seen.insert(trade_id) {
                continue;
            }
            // `CASH` trades are FX conversions between the account's
            // currency sleeves, not security trades. Importing them as buys
            // would invent assets named like `GBP.JPY`; currency movements
            // are covered by the cash-flow provider instead.
            if trade.sec_type.as_deref() == Some("CASH") {
                skipped_fx += 1;
                continue;
            }
            if let Some(activity) = map_trade(&trade) {
                activities.push(activity);
            }
        }
    }

    if total_windows > 0 && failures.len() == total_windows {
        return Err(Error::Unexpected(format!(
            "every IBKR trade window failed ({})",
            failures.join(", ")
        )));
    }
    if !failures.is_empty() {
        log::warn!(
            "IBKR MCP: {}/{} trade windows failed ({}); continuing with {} trades",
            failures.len(),
            total_windows,
            failures.join(", "),
            activities.len()
        );
    }

    activities.sort_by(|a, b| a.trade_date.cmp(&b.trade_date));
    if skipped_fx > 0 {
        log::info!("IBKR MCP: skipped {skipped_fx} FX conversion trades");
    }
    log::info!("IBKR MCP: loaded {} unique trades", activities.len());

    Ok(activities)
}

/// Map an IBKR trade onto the universal activity shape.
fn map_trade(trade: &IbkrTrade) -> Option<AccountUniversalActivity> {
    let trade_id = trade.trade_id.clone()?;
    let symbol = trade.symbol.clone()?;

    let activity_type = match trade.side.as_deref() {
        Some("BUY") => "BUY",
        Some("SELL") => "SELL",
        _ => return None,
    };

    let currency = trade.currency.clone().map(|code| {
        AccountUniversalActivityCurrency {
            code: Some(code),
            ..Default::default()
        }
    });

    Some(AccountUniversalActivity {
        id: Some(trade_id.clone()),
        symbol: Some(AccountUniversalActivitySymbol {
            symbol: Some(symbol.clone()),
            raw_symbol: Some(symbol),
            description: trade.company_name.clone(),
            currency: currency.clone(),
            ..Default::default()
        }),
        price: trade.price,
        units: trade.size.map(f64::abs),
        amount: trade.net_amount,
        currency,
        activity_type: Some(activity_type.to_string()),
        raw_type: trade.side.clone(),
        description: trade.description.clone(),
        trade_date: trade.trade_time.clone(),
        settlement_date: trade.trade_time.clone(),
        fee: trade.commission,
        institution: Some("Interactive Brokers".to_string()),
        external_reference_id: Some(trade_id.clone()),
        provider_type: Some(IBKR_SOURCE_SYSTEM.to_string()),
        source_system: Some(IBKR_SOURCE_SYSTEM.to_string()),
        source_record_id: Some(trade_id),
        ..Default::default()
    })
}

/// Compare an RFC 3339 trade instant against a `YYYY-MM-DD` bound.
fn within_range(trade_date: Option<&str>, start: Option<&str>, end: Option<&str>) -> bool {
    let Some(date) = trade_date.and_then(|d| d.get(..10)) else {
        // Undated activities cannot be filtered; keep them so they are not
        // silently dropped from a bounded sync.
        return true;
    };
    if let Some(start) = start {
        if date < start {
            return false;
        }
    }
    if let Some(end) = end {
        if date > end {
            return false;
        }
    }
    true
}

#[async_trait]
impl BrokerApiClient for IbkrMcpClient {
    async fn list_connections(&self) -> Result<Vec<BrokerConnection>> {
        Ok(vec![BrokerConnection {
            id: IBKR_CONNECTION_ID.to_string(),
            brokerage: Some(BrokerConnectionBrokerage {
                id: Some(IBKR_BROKERAGE_ID.to_string()),
                slug: Some(IBKR_BROKERAGE_SLUG.to_string()),
                name: Some("Interactive Brokers".to_string()),
                display_name: Some("Interactive Brokers".to_string()),
                aws_s3_logo_url: None,
                aws_s3_square_logo_url: None,
            }),
            connection_type: Some("read".to_string()),
            status: Some("connected".to_string()),
            disabled: false,
            disabled_date: None,
            updated_at: None,
            name: Some("Interactive Brokers (MCP)".to_string()),
        }])
    }

    async fn list_accounts(
        &self,
        authorization_ids: Option<Vec<String>>,
    ) -> Result<Vec<BrokerAccount>> {
        if let Some(ids) = authorization_ids {
            if !ids.iter().any(|id| id == IBKR_CONNECTION_ID) {
                return Ok(vec![]);
            }
        }

        let summary = self.account_summary().await?;
        let currency = self
            .base_currency()
            .await
            .or_else(|| summary.currency.clone());

        Ok(vec![BrokerAccount {
            id: Some(IBKR_ACCOUNT_ID.to_string()),
            name: Some("Interactive Brokers".to_string()),
            account_number: Some(IBKR_ACCOUNT_ID.to_string()),
            account_type: Some("MARGIN".to_string()),
            currency: currency.clone(),
            balance: Some(BrokerAccountBalance {
                total: Some(BrokerBalanceTotal {
                    amount: summary.net_liquidation,
                    currency,
                }),
            }),
            meta: None,
            owner: Some(AccountOwner {
                is_own_account: true,
                ..Default::default()
            }),
            brokerage_authorization: Some(IBKR_CONNECTION_ID.to_string()),
            institution_name: Some("Interactive Brokers".to_string()),
            created_date: None,
            sync_status: None,
            status: Some("open".to_string()),
            raw_type: Some("MARGIN".to_string()),
            is_paper: false,
            sync_enabled: true,
            shared_with_household: false,
        }])
    }

    async fn list_brokerages(&self) -> Result<Vec<BrokerBrokerage>> {
        Ok(vec![BrokerBrokerage {
            id: Some(IBKR_BROKERAGE_ID.to_string()),
            slug: Some(IBKR_BROKERAGE_SLUG.to_string()),
            name: Some("Interactive Brokers".to_string()),
            display_name: Some("Interactive Brokers".to_string()),
            url: Some("https://www.interactivebrokers.com".to_string()),
            enabled: true,
        }])
    }

    async fn get_account_activities(
        &self,
        account_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        offset: Option<i64>,
        limit: Option<i64>,
    ) -> Result<PaginatedUniversalActivity> {
        if account_id != IBKR_ACCOUNT_ID {
            return Ok(PaginatedUniversalActivity::default());
        }

        let all = self.load_trades().await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|activity| within_range(activity.trade_date.as_deref(), start_date, end_date))
            .collect();

        let total = filtered.len() as i64;
        let offset = offset.unwrap_or(0).max(0);
        let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).max(1);
        let page: Vec<_> = filtered
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        let has_more = offset + (page.len() as i64) < total;

        Ok(PaginatedUniversalActivity {
            data: page,
            pagination: Some(PaginationDetails {
                offset: Some(offset),
                limit: Some(limit),
                total: Some(total),
                has_more: Some(has_more),
            }),
        })
    }

    async fn get_account_holdings(&self, account_id: &str) -> Result<BrokerHoldingsResponse> {
        if account_id != IBKR_ACCOUNT_ID {
            return Ok(BrokerHoldingsResponse::default());
        }

        let positions_value = self.mcp.call_tool("get_account_positions", json!({})).await?;
        let positions: IbkrPositionsResponse = serde_json::from_value(positions_value)
            .map_err(|e| Error::Unexpected(format!("Unexpected IBKR positions shape: {e}")))?;

        let balances_value = self.mcp.call_tool("get_account_balances", json!({})).await?;
        let balances: IbkrBalancesResponse = serde_json::from_value(balances_value)
            .map_err(|e| Error::Unexpected(format!("Unexpected IBKR balances shape: {e}")))?;

        let mapped_positions: Vec<HoldingsPosition> = positions
            .positions
            .into_iter()
            .filter(|p| p.position.unwrap_or(0.0) != 0.0)
            .map(|p| {
                let currency = p.currency.clone().map(|code| HoldingsCurrency {
                    code: Some(code),
                    ..Default::default()
                });
                let ticker = p.contract_description.clone();
                HoldingsPosition {
                    symbol: Some(HoldingsSymbol {
                        symbol: Some(HoldingsInnerSymbol {
                            symbol: ticker.clone(),
                            raw_symbol: ticker.clone(),
                            description: ticker.clone(),
                            name: ticker,
                            currency: currency.clone(),
                            ..Default::default()
                        }),
                        id: p.contract_id.map(|id| id.to_string()),
                        description: p.contract_description.clone(),
                    }),
                    units: p.position,
                    price: p.market_price,
                    open_pnl: p.unrealized_pnl,
                    average_purchase_price: p.average_price,
                    currency,
                    cash_equivalent: Some(false),
                }
            })
            .collect();

        let mapped_balances: Vec<HoldingsBalance> = balances
            .balances
            .into_iter()
            // The `BASE` row is the base-currency aggregate of every other row;
            // importing it alongside them would double-count the cash.
            .filter(|b| !b.is_base_aggregate())
            .filter_map(|b| {
                let code = b.currency.clone()?;
                Some(HoldingsBalance {
                    currency: Some(HoldingsCurrency {
                        code: Some(code),
                        ..Default::default()
                    }),
                    cash: b.cash_balance,
                    buying_power: None,
                })
            })
            .collect();

        log::info!(
            "IBKR MCP: {} open positions, {} cash balances",
            mapped_positions.len(),
            mapped_balances.len()
        );

        Ok(BrokerHoldingsResponse {
            account: Some(HoldingsAccount {
                id: Some(IBKR_ACCOUNT_ID.to_string()),
                name: Some("Interactive Brokers".to_string()),
                number: Some(IBKR_ACCOUNT_ID.to_string()),
                raw_type: Some("MARGIN".to_string()),
            }),
            balances: Some(mapped_balances),
            positions: Some(mapped_positions),
            option_positions: Some(vec![]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use super::super::models::IbkrPerformancePeriod;

    fn trade(id: &str, side: &str, sec_type: &str) -> IbkrTrade {
        IbkrTrade {
            trade_id: Some(id.to_string()),
            symbol: Some("GM".to_string()),
            company_name: Some("GENERAL MOTORS CO".to_string()),
            sec_type: Some(sec_type.to_string()),
            currency: Some("USD".to_string()),
            side: Some(side.to_string()),
            size: Some(1.5),
            price: Some(81.69),
            trade_time: Some("2026-10-07T13:30:09Z".to_string()),
            commission: Some(0.35),
            net_amount: Some(122.54),
            ..Default::default()
        }
    }

    #[test]
    fn maps_a_buy_trade() {
        let activity = map_trade(&trade("t1", "BUY", "STK")).expect("mapped");
        assert_eq!(activity.activity_type.as_deref(), Some("BUY"));
        assert_eq!(activity.units, Some(1.5));
        assert_eq!(activity.fee, Some(0.35));
        assert_eq!(activity.source_record_id.as_deref(), Some("t1"));
        assert_eq!(
            activity.symbol.unwrap().raw_symbol.as_deref(),
            Some("GM")
        );
    }

    #[test]
    fn rejects_trades_without_a_recognised_side() {
        assert!(map_trade(&trade("t2", "EXCH", "STK")).is_none());
    }

    #[test]
    fn date_filter_uses_the_date_part_of_the_instant() {
        let date = Some("2026-10-07T13:30:09Z");
        assert!(within_range(date, Some("2026-10-07"), Some("2026-10-07")));
        assert!(!within_range(date, Some("2026-10-08"), None));
        assert!(!within_range(date, None, Some("2026-10-06")));
        assert!(within_range(date, None, None));
    }

    #[test]
    fn undated_activities_are_kept() {
        assert!(within_range(None, Some("2026-01-01"), Some("2026-12-31")));
    }

    fn window(trades: Vec<IbkrTrade>) -> IbkrTradesResponse {
        IbkrTradesResponse { trades }
    }

    /// A single flaky window used to wipe the entire activity import, which in
    /// HOLDINGS mode surfaced only as a warning behind a successful positions
    /// sync — holdings present, activity list empty.
    #[test]
    fn one_failed_window_does_not_lose_the_others() {
        let merged = merge_trade_windows(vec![
            ("LAST_QUARTER", Err("boom".to_string())),
            ("YEAR_TO_DATE", Ok(window(vec![trade("t1", "BUY", "STK")]))),
        ])
        .expect("partial failure must not be fatal");

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source_record_id.as_deref(), Some("t1"));
    }

    #[test]
    fn a_total_blackout_is_still_an_error() {
        let merged = merge_trade_windows(vec![
            ("LAST_QUARTER", Err("boom".to_string())),
            ("YEAR_TO_DATE", Err("boom".to_string())),
        ]);

        assert!(merged.is_err(), "every window failing must surface");
    }

    /// The windows overlap, so the same trade arrives several times.
    #[test]
    fn overlapping_windows_are_deduplicated_and_fx_is_dropped() {
        let merged = merge_trade_windows(vec![
            (
                "LAST_QUARTER",
                Ok(window(vec![
                    trade("t1", "BUY", "STK"),
                    trade("fx1", "BUY", "CASH"),
                ])),
            ),
            (
                "YEAR_TO_DATE",
                Ok(window(vec![
                    trade("t1", "BUY", "STK"),
                    trade("t2", "SELL", "STK"),
                ])),
            ),
        ])
        .expect("merged");

        let ids: Vec<_> = merged
            .iter()
            .filter_map(|a| a.source_record_id.clone())
            .collect();
        assert_eq!(ids, vec!["t1".to_string(), "t2".to_string()]);
    }

    fn perf_account(
        periods: Vec<(&str, Vec<&str>, Vec<f64>)>,
    ) -> IbkrPerformanceAccount {
        let map = periods
            .into_iter()
            .map(|(name, dates, nav)| {
                (
                    name.to_string(),
                    IbkrPerformancePeriod {
                        dates: Some(dates.into_iter().map(String::from).collect()),
                        nav: Some(nav),
                        cps: None,
                    },
                )
            })
            .collect();

        IbkrPerformanceAccount {
            periods: Some(map),
            ..Default::default()
        }
    }

    #[test]
    fn nav_series_keeps_the_longest_window() {
        let account = perf_account(vec![
            ("7D", vec!["20261006", "20261007"], vec![10.0, 11.0]),
            (
                "1Y",
                vec!["20261005", "20261006", "20261007"],
                vec![9.0, 10.0, 11.0],
            ),
        ]);

        let series = longest_nav_series(&account);
        assert_eq!(series.len(), 3);
        assert_eq!(
            series.first().unwrap().0,
            NaiveDate::from_ymd_opt(2026, 10, 5).unwrap()
        );
    }

    #[test]
    fn nav_series_skips_unparsable_dates_and_sorts() {
        let account = perf_account(vec![(
            "1Y",
            vec!["20261007", "not-a-date", "20261005"],
            vec![11.0, 99.0, 9.0],
        )]);

        let series = longest_nav_series(&account);
        assert_eq!(
            series,
            vec![
                (NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(), dec!(9)),
                (NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(), dec!(11)),
            ]
        );
    }

    #[test]
    fn nav_series_is_empty_without_periods() {
        assert!(longest_nav_series(&IbkrPerformanceAccount::default()).is_empty());
    }
}
