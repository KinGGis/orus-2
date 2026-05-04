//! Portfolio Summary tool - comprehensive portfolio overview.
//!
//! This tool provides a high-level summary of the portfolio similar to Thot's
//! get_portfolio_summary function in Orus. It combines:
//! - Total NAV and daily change
//! - Asset allocation breakdown
//! - Top holdings
//! - Cash position summary
//! - Performance metrics

use chrono::Datelike;
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

// ============================================================================
// Tool Arguments (LLM Input)
// ============================================================================

/// Arguments for the portfolio_summary tool.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummaryArgs {
    /// Account ID, or "TOTAL" for all accounts.
    #[serde(default = "default_account_id")]
    pub account_id: String,
}

fn default_account_id() -> String {
    "TOTAL".to_string()
}

// ============================================================================
// Output Types
// ============================================================================

/// Output for the portfolio_summary tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummaryOutput {
    /// Account scope (account name or "All Accounts").
    pub account_scope: String,

    /// Base currency.
    pub currency: String,

    /// Total portfolio value (NAV).
    pub total_value: f64,

    /// Total invested (cost basis).
    pub total_invested: f64,

    /// Total unrealized gain/loss.
    pub total_gain_loss: f64,

    /// Total gain/loss as percentage.
    pub total_gain_loss_pct: f64,

    /// Daily change in value.
    pub day_change: f64,

    /// Daily change as percentage.
    pub day_change_pct: f64,

    /// Number of positions (excluding cash).
    pub position_count: usize,

    /// Asset allocation breakdown.
    pub asset_allocation: Vec<AllocationItem>,

    /// Top 5 holdings by value.
    pub top_holdings: Vec<TopHolding>,

    /// Cash positions summary.
    pub cash_summary: CashSummary,

    /// YTD performance if available.
    pub ytd_return_pct: Option<f64>,
}

/// Asset allocation item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllocationItem {
    pub category: String,
    pub value: f64,
    pub weight: f64,
}

/// Top holding summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopHolding {
    pub symbol: String,
    pub name: Option<String>,
    pub value: f64,
    pub weight: f64,
    pub day_change_pct: Option<f64>,
    pub unrealized_gain_pct: Option<f64>,
}

/// Cash position summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashSummary {
    /// Total cash value in base currency.
    pub total_value: f64,

    /// Cash as percentage of portfolio.
    pub weight: f64,

    /// Cash positions by currency.
    pub by_currency: Vec<CashByCurrency>,
}

/// Cash held in a specific currency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashByCurrency {
    pub currency: String,
    pub local_value: f64,
    pub base_value: f64,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to get a comprehensive portfolio summary.
pub struct PortfolioSummaryTool<E: AiEnvironment> {
    env: Arc<E>,
    base_currency: String,
}

impl<E: AiEnvironment> PortfolioSummaryTool<E> {
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self { env, base_currency }
    }
}

impl<E: AiEnvironment> Clone for PortfolioSummaryTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            base_currency: self.base_currency.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for PortfolioSummaryTool<E> {
    const NAME: &'static str = "get_portfolio_summary";

    type Error = AiError;
    type Args = PortfolioSummaryArgs;
    type Output = PortfolioSummaryOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get a comprehensive portfolio summary including total value, daily change, asset allocation, top holdings, and cash positions. Use this for quick portfolio overview questions like 'how is my portfolio doing?' or 'give me a summary of my investments'.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountId": {
                        "type": "string",
                        "description": "Account ID to summarize, or 'TOTAL' for all accounts",
                        "default": "TOTAL"
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let account_id = &args.account_id;

        // Fetch holdings
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(account_id, &self.base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        // Separate cash and non-cash holdings
        let (cash_holdings, security_holdings): (Vec<_>, Vec<_>) = holdings
            .into_iter()
            .partition(|h| h.holding_type == wealthfolio_core::holdings::HoldingType::Cash);

        // Calculate totals
        let securities_value: f64 = security_holdings
            .iter()
            .map(|h| h.market_value.base.to_f64().unwrap_or(0.0))
            .sum();

        let cash_value: f64 = cash_holdings
            .iter()
            .map(|h| h.market_value.base.to_f64().unwrap_or(0.0))
            .sum();

        let total_value = securities_value + cash_value;

        // Calculate total cost basis and gain/loss
        let total_invested: f64 = security_holdings
            .iter()
            .filter_map(|h| h.cost_basis.as_ref().map(|c| c.base.to_f64().unwrap_or(0.0)))
            .sum();

        let total_gain_loss = securities_value - total_invested;
        let total_gain_loss_pct = if total_invested > 0.0 {
            (total_gain_loss / total_invested) * 100.0
        } else {
            0.0
        };

        // Calculate day change (weighted average of day changes)
        let day_change: f64 = security_holdings
            .iter()
            .filter_map(|h| {
                h.day_change_pct.and_then(|pct| {
                    let value = h.market_value.base.to_f64().unwrap_or(0.0);
                    let prev_value = value / (1.0 + pct.to_f64().unwrap_or(0.0) / 100.0);
                    Some(value - prev_value)
                })
            })
            .sum();

        let day_change_pct = if (total_value - day_change) > 0.0 {
            (day_change / (total_value - day_change)) * 100.0
        } else {
            0.0
        };

        // Build asset allocation by type
        let mut allocation_map: HashMap<String, f64> = HashMap::new();
        for h in &security_holdings {
            // Get asset class from classifications if available
            let asset_type = h
                .instrument
                .as_ref()
                .and_then(|i| i.classifications.as_ref())
                .and_then(|c| c.asset_classes.first())
                .map(|cw| cw.category.name.clone())
                .unwrap_or_else(|| "Other".to_string());
            let value = h.market_value.base.to_f64().unwrap_or(0.0);
            *allocation_map.entry(asset_type).or_insert(0.0) += value;
        }
        // Add cash
        if cash_value > 0.0 {
            allocation_map.insert("Cash".to_string(), cash_value);
        }

        let mut asset_allocation: Vec<AllocationItem> = allocation_map
            .into_iter()
            .map(|(category, value)| AllocationItem {
                category,
                value,
                weight: if total_value > 0.0 {
                    (value / total_value) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        asset_allocation.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

        // Build top 5 holdings
        let mut sorted_holdings = security_holdings.clone();
        sorted_holdings.sort_by(|a, b| {
            let av = a.market_value.base.to_f64().unwrap_or(0.0);
            let bv = b.market_value.base.to_f64().unwrap_or(0.0);
            bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
        });

        let top_holdings: Vec<TopHolding> = sorted_holdings
            .iter()
            .take(5)
            .map(|h| {
                let value = h.market_value.base.to_f64().unwrap_or(0.0);
                let (symbol, name) = h
                    .instrument
                    .as_ref()
                    .map(|i| (i.symbol.clone(), i.name.clone()))
                    .unwrap_or_else(|| ("UNKNOWN".to_string(), None));

                TopHolding {
                    symbol,
                    name,
                    value,
                    weight: if total_value > 0.0 {
                        (value / total_value) * 100.0
                    } else {
                        0.0
                    },
                    day_change_pct: h.day_change_pct.and_then(|d| d.to_f64()),
                    unrealized_gain_pct: h.unrealized_gain_pct.and_then(|d| d.to_f64()),
                }
            })
            .collect();

        // Build cash summary
        let cash_by_currency: Vec<CashByCurrency> = cash_holdings
            .iter()
            .map(|h| CashByCurrency {
                currency: h.local_currency.clone(),
                local_value: h.market_value.local.to_f64().unwrap_or(0.0),
                base_value: h.market_value.base.to_f64().unwrap_or(0.0),
            })
            .collect();

        let cash_summary = CashSummary {
            total_value: cash_value,
            weight: if total_value > 0.0 {
                (cash_value / total_value) * 100.0
            } else {
                0.0
            },
            by_currency: cash_by_currency,
        };

        // Get YTD performance if available
        // Calculate YTD date range
        let now = chrono::Local::now().date_naive();
        let year_start = chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1);
        
        let ytd_return_pct = if let Some(start_date) = year_start {
            self.env
                .performance_service()
                .calculate_performance_summary(
                    if account_id == "TOTAL" { "portfolio" } else { "account" },
                    account_id,
                    Some(start_date),
                    Some(now),
                    None,
                )
                .await
                .ok()
                .and_then(|p| p.period_return.to_f64())
        } else {
            None
        };

        // Get account scope name
        let account_scope = if account_id == "TOTAL" {
            "All Accounts".to_string()
        } else {
            self.env
                .account_service()
                .list_accounts(None, None, None)
                .ok()
                .and_then(|accounts| accounts.into_iter().find(|a| a.id == *account_id).map(|a| a.name))
                .unwrap_or_else(|| account_id.clone())
        };

        Ok(PortfolioSummaryOutput {
            account_scope,
            currency: self.base_currency.clone(),
            total_value,
            total_invested,
            total_gain_loss,
            total_gain_loss_pct,
            day_change,
            day_change_pct,
            position_count: security_holdings.len(),
            asset_allocation,
            top_holdings,
            cash_summary,
            ytd_return_pct,
        })
    }
}
