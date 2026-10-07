//! Payload types returned by the IBKR MCP server (`ibkr-cpapi-mcp`).
//!
//! Field names mirror the JSON exactly; every field is optional because the
//! server omits keys rather than sending nulls (for example `asset_class` is
//! absent on non-US listings).

use serde::Deserialize;

/// Response of the `get_account_summary` tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrAccountSummary {
    pub currency: Option<String>,
    pub net_liquidation: Option<f64>,
    pub equity_with_loan_value: Option<f64>,
    pub buying_power: Option<f64>,
    pub gross_position_value: Option<f64>,
    pub total_cash_value: Option<f64>,
    pub available_funds: Option<f64>,
    pub excess_liquidity: Option<f64>,
}

/// Response of the `get_account_balances` tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrBalancesResponse {
    #[serde(default)]
    pub balances: Vec<IbkrBalance>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrBalance {
    /// Currency code, or the literal `BASE` for the account-level aggregate.
    pub currency: Option<String>,
    pub cash_balance: Option<f64>,
    pub settled_cash: Option<f64>,
    pub net_liquidation_value: Option<f64>,
    pub stock_market_value: Option<f64>,
    pub exchange_rate: Option<f64>,
}

impl IbkrBalance {
    /// The synthetic `BASE` row aggregates every currency and must not be
    /// ingested as a cash balance of its own.
    pub fn is_base_aggregate(&self) -> bool {
        self.currency.as_deref() == Some("BASE")
    }
}

/// Response of the `get_account_positions` tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPositionsResponse {
    #[serde(default)]
    pub positions: Vec<IbkrPosition>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPosition {
    pub contract_id: Option<i64>,
    pub contract_description: Option<String>,
    /// Signed quantity: negative for short positions.
    pub position: Option<f64>,
    pub market_price: Option<f64>,
    pub market_value: Option<f64>,
    pub currency: Option<String>,
    pub average_price: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub asset_class: Option<String>,
}

/// Response of the `get_account_trades` tool.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrTradesResponse {
    #[serde(default)]
    pub trades: Vec<IbkrTrade>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrTrade {
    pub trade_id: Option<String>,
    pub symbol: Option<String>,
    pub company_name: Option<String>,
    /// `STK` for equities/ETFs, `CASH` for FX conversions.
    pub sec_type: Option<String>,
    pub currency: Option<String>,
    pub side: Option<String>,
    pub size: Option<f64>,
    pub price: Option<f64>,
    pub order_type: Option<String>,
    pub description: Option<String>,
    /// RFC 3339 UTC instant, e.g. `2026-10-07T13:30:09Z`.
    pub trade_time: Option<String>,
    pub exchange: Option<String>,
    pub commission: Option<f64>,
    pub net_amount: Option<f64>,
    pub realized_pnl: Option<f64>,
}

/// Rolling windows accepted by `get_account_trades`.
///
/// IBKR exposes at most the year-to-date window plus the four preceding
/// completed quarters, so this is the deepest history the MCP server can
/// return. Ordered oldest-first so a full sweep yields chronological data.
pub const TRADE_PERIODS: [&str; 5] = [
    "FOUR_QUARTERS_AGO",
    "THREE_QUARTERS_AGO",
    "TWO_QUARTERS_AGO",
    "LAST_QUARTER",
    "YEAR_TO_DATE",
];

/// Response of `get_pa_performance_all_periods`, used for the base currency
/// and the account inception date.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPerformanceResponse {
    pub accounts: Option<IbkrPerformanceAccounts>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPerformanceAccounts {
    pub account: Option<IbkrPerformanceAccount>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPerformanceAccount {
    pub base_currency: Option<String>,
    /// Account inception date in `yyyymmdd`.
    pub start: Option<String>,
    pub end: Option<String>,
    /// Keyed by window label (`1D`, `7D`, `MTD`, `1M`, `YTD`, `1Y`).
    pub periods: Option<std::collections::HashMap<String, IbkrPerformancePeriod>>,
}

/// One performance window. `dates`, `nav` and `cps` are parallel arrays of
/// equal length; `dates` are `yyyymmdd` strings and `cps` holds cumulative
/// returns as fractions measured from the start of the window.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IbkrPerformancePeriod {
    pub dates: Option<Vec<String>>,
    pub nav: Option<Vec<f64>>,
    pub cps: Option<Vec<f64>>,
}
