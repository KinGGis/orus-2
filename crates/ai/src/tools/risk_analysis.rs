//! Risk Analysis tool - portfolio concentration and risk metrics.
//!
//! This tool provides risk analysis capabilities similar to Thot in Orus:
//! - Concentration by country/region
//! - Concentration by sector/industry
//! - Top position weights
//! - Concentration risk indicators

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

/// Arguments for the risk_analysis tool.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskAnalysisArgs {
    /// Account ID, or "TOTAL" for all accounts.
    #[serde(default = "default_account_id")]
    pub account_id: String,

    /// Number of top positions to include (default: 10).
    #[serde(default = "default_top_n")]
    pub top_n: usize,
}

fn default_account_id() -> String {
    "TOTAL".to_string()
}

fn default_top_n() -> usize {
    10
}

// ============================================================================
// Output Types
// ============================================================================

/// Output for the risk_analysis tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskAnalysisOutput {
    /// Total portfolio value in base currency.
    pub total_value: f64,

    /// Base currency.
    pub currency: String,

    /// Account scope (account name or "All Accounts").
    pub account_scope: String,

    /// Concentration breakdown by sector.
    pub sector_concentration: Vec<ConcentrationItem>,

    /// Concentration breakdown by country/region.
    pub country_concentration: Vec<ConcentrationItem>,

    /// Concentration breakdown by asset type.
    pub asset_type_concentration: Vec<ConcentrationItem>,

    /// Top positions by weight.
    pub top_positions: Vec<TopPosition>,

    /// Risk indicators.
    pub risk_indicators: RiskIndicators,
}

/// Concentration item showing category and weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcentrationItem {
    /// Category name (sector, country, etc.).
    pub category: String,

    /// Value in base currency.
    pub value: f64,

    /// Weight as percentage (0-100).
    pub weight: f64,
}

/// Top position in the portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopPosition {
    /// Symbol/ticker.
    pub symbol: String,

    /// Asset name.
    pub name: Option<String>,

    /// Market value in base currency.
    pub value: f64,

    /// Weight as percentage (0-100).
    pub weight: f64,

    /// Sector if available.
    pub sector: Option<String>,

    /// Country if available.
    pub country: Option<String>,
}

/// Portfolio risk indicators.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskIndicators {
    /// Weight of the largest position (concentration risk).
    pub largest_position_weight: f64,

    /// Weight of top 5 positions combined.
    pub top5_weight: f64,

    /// Weight of top 10 positions combined.
    pub top10_weight: f64,

    /// Number of distinct positions.
    pub position_count: usize,

    /// Number of distinct sectors.
    pub sector_count: usize,

    /// Number of distinct countries.
    pub country_count: usize,

    /// Herfindahl-Hirschman Index (measure of concentration, 0-10000).
    pub hhi: f64,

    /// Diversification assessment: "Low", "Moderate", "Good", "Excellent".
    pub diversification_rating: String,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to analyze portfolio risk and concentration.
pub struct RiskAnalysisTool<E: AiEnvironment> {
    env: Arc<E>,
    base_currency: String,
}

impl<E: AiEnvironment> RiskAnalysisTool<E> {
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self { env, base_currency }
    }
}

impl<E: AiEnvironment> Clone for RiskAnalysisTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
            base_currency: self.base_currency.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for RiskAnalysisTool<E> {
    const NAME: &'static str = "get_risk_analysis";

    type Error = AiError;
    type Args = RiskAnalysisArgs;
    type Output = RiskAnalysisOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Analyze portfolio risk and concentration. Returns sector/country concentration, top positions, and risk indicators like HHI (Herfindahl-Hirschman Index) and diversification rating. Use this when the user asks about risk, concentration, diversification, or portfolio exposure.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "accountId": {
                        "type": "string",
                        "description": "Account ID to analyze, or 'TOTAL' for all accounts",
                        "default": "TOTAL"
                    },
                    "topN": {
                        "type": "integer",
                        "description": "Number of top positions to include",
                        "default": 10
                    }
                },
                "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let account_id = &args.account_id;
        let top_n = args.top_n.max(1).min(50);

        // Fetch holdings
        let holdings = self
            .env
            .holdings_service()
            .get_holdings(account_id, &self.base_currency)
            .await
            .map_err(|e| AiError::ToolExecutionFailed(e.to_string()))?;

        // Filter out cash positions
        let holdings: Vec<_> = holdings
            .into_iter()
            .filter(|h| h.holding_type != wealthfolio_core::holdings::HoldingType::Cash)
            .collect();

        let total_value: f64 = holdings
            .iter()
            .map(|h| h.market_value.base.to_f64().unwrap_or(0.0))
            .sum();

        if total_value == 0.0 {
            return Ok(RiskAnalysisOutput {
                total_value: 0.0,
                currency: self.base_currency.clone(),
                account_scope: if account_id == "TOTAL" {
                    "All Accounts".to_string()
                } else {
                    account_id.clone()
                },
                sector_concentration: vec![],
                country_concentration: vec![],
                asset_type_concentration: vec![],
                top_positions: vec![],
                risk_indicators: RiskIndicators {
                    largest_position_weight: 0.0,
                    top5_weight: 0.0,
                    top10_weight: 0.0,
                    position_count: 0,
                    sector_count: 0,
                    country_count: 0,
                    hhi: 0.0,
                    diversification_rating: "N/A".to_string(),
                },
            });
        }

        // Build concentration maps
        let mut sector_map: HashMap<String, f64> = HashMap::new();
        let mut country_map: HashMap<String, f64> = HashMap::new();
        let mut asset_type_map: HashMap<String, f64> = HashMap::new();

        // Collect positions for sorting
        let mut positions: Vec<(String, Option<String>, f64, Option<String>, Option<String>)> = Vec::new();

        for h in &holdings {
            let value = h.market_value.base.to_f64().unwrap_or(0.0);
            let instrument = h.instrument.as_ref();

            // Extract symbol and name
            let (symbol, name) = instrument
                .map(|i| (i.symbol.clone(), i.name.clone()))
                .unwrap_or_else(|| ("UNKNOWN".to_string(), None));

            // Extract sector from classifications
            let sector = instrument
                .and_then(|i| i.classifications.as_ref())
                .and_then(|c| c.sectors.first())
                .map(|cw| cw.category.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Extract country/region from classifications
            let country = instrument
                .and_then(|i| i.classifications.as_ref())
                .and_then(|c| c.regions.first())
                .map(|cw| cw.category.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Asset type
            let asset_type = match h.holding_type {
                wealthfolio_core::holdings::HoldingType::Cash => "Cash",
                wealthfolio_core::holdings::HoldingType::Security => "Security",
                wealthfolio_core::holdings::HoldingType::AlternativeAsset => "Alternative",
            };

            // Accumulate concentrations
            *sector_map.entry(sector.clone()).or_insert(0.0) += value;
            *country_map.entry(country.clone()).or_insert(0.0) += value;
            *asset_type_map.entry(asset_type.to_string()).or_insert(0.0) += value;

            positions.push((symbol, name, value, Some(sector), Some(country)));
        }

        // Sort positions by value descending
        positions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate top positions
        let top_positions: Vec<TopPosition> = positions
            .iter()
            .take(top_n)
            .map(|(symbol, name, value, sector, country)| TopPosition {
                symbol: symbol.clone(),
                name: name.clone(),
                value: *value,
                weight: (*value / total_value) * 100.0,
                sector: sector.clone(),
                country: country.clone(),
            })
            .collect();

        // Convert concentration maps to sorted vectors
        let sector_concentration = map_to_concentration(&sector_map, total_value);
        let country_concentration = map_to_concentration(&country_map, total_value);
        let asset_type_concentration = map_to_concentration(&asset_type_map, total_value);

        // Calculate risk indicators
        let position_weights: Vec<f64> = positions.iter().map(|(_, _, v, _, _)| *v / total_value).collect();

        let largest_position_weight = position_weights.first().copied().unwrap_or(0.0) * 100.0;
        let top5_weight: f64 = position_weights.iter().take(5).sum::<f64>() * 100.0;
        let top10_weight: f64 = position_weights.iter().take(10).sum::<f64>() * 100.0;

        // Calculate HHI (Herfindahl-Hirschman Index)
        // HHI = sum of (market_share * 100)^2 for each position
        // Range: ~0 (highly diversified) to 10000 (single position)
        let hhi: f64 = position_weights.iter().map(|w| (w * 100.0).powi(2)).sum();

        // Determine diversification rating based on HHI and position count
        let diversification_rating = if holdings.len() < 5 {
            "Low"
        } else if hhi > 2500.0 {
            "Low"
        } else if hhi > 1500.0 {
            "Moderate"
        } else if hhi > 1000.0 {
            "Good"
        } else {
            "Excellent"
        };

        let risk_indicators = RiskIndicators {
            largest_position_weight,
            top5_weight,
            top10_weight,
            position_count: holdings.len(),
            sector_count: sector_map.len(),
            country_count: country_map.len(),
            hhi,
            diversification_rating: diversification_rating.to_string(),
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

        Ok(RiskAnalysisOutput {
            total_value,
            currency: self.base_currency.clone(),
            account_scope,
            sector_concentration,
            country_concentration,
            asset_type_concentration,
            top_positions,
            risk_indicators,
        })
    }
}

/// Convert a HashMap to a sorted vector of ConcentrationItems.
fn map_to_concentration(map: &HashMap<String, f64>, total: f64) -> Vec<ConcentrationItem> {
    let mut items: Vec<ConcentrationItem> = map
        .iter()
        .map(|(category, value)| ConcentrationItem {
            category: category.clone(),
            value: *value,
            weight: (*value / total) * 100.0,
        })
        .collect();

    // Sort by value descending
    items.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

    items
}
