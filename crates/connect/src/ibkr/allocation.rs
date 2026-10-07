//! Portfolio allocations sourced from IBKR's `get_pa_allocation` tool.
//!
//! The local allocation service derives categories by joining holdings against
//! our taxonomy tables. That join depends on every asset being classified, and
//! unclassified assets silently fall into "Unknown", which is what made the
//! allocation cards unusable. IBKR already classifies every position it holds,
//! so for an IBKR account we take their breakdown verbatim.

use std::collections::HashMap;

use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use serde_json::json;

use wealthfolio_core::portfolio::allocation::{
    CategoryAllocation, PortfolioAllocations, TaxonomyAllocation,
};

use super::mcp::McpClient;
use wealthfolio_core::errors::{Error, Result};

/// Colors mirror the defaults of [`PortfolioAllocations`] so the IBKR-backed
/// cards look identical to the locally computed ones.
const ASSET_CLASS_COLOR: &str = "#879a39";
const SECTOR_COLOR: &str = "#da702c";
const REGION_COLOR: &str = "#8b7ec8";
const INSTRUMENT_COLOR: &str = "#4385be";
const COUNTRY_COLOR: &str = "#3aa99f";

/// Palette used to colour individual categories, which IBKR does not provide.
const CATEGORY_PALETTE: [&str; 12] = [
    "#4385be", "#879a39", "#da702c", "#8b7ec8", "#d14d41", "#3aa99f", "#ce5d97", "#a0a000",
    "#5e409d", "#bc5215", "#66800b", "#24837b",
];

#[derive(Debug, Deserialize)]
pub struct AllocationResponse {
    #[serde(default)]
    pub allocations: HashMap<String, AllocationDimension>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationDimension {
    #[serde(default)]
    pub long_positions: Option<AllocationBucket>,
    /// Only present when the account actually holds shorts in this dimension.
    #[serde(default)]
    pub short_positions: Option<AllocationBucket>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationBucket {
    #[serde(default)]
    pub total: Option<AllocationTotal>,
    #[serde(default)]
    pub items: Vec<AllocationItem>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationTotal {
    #[serde(default)]
    pub nav: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct AllocationItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nav: Option<f64>,
}

fn to_decimal(value: Option<f64>) -> Decimal {
    value
        .and_then(Decimal::from_f64)
        .unwrap_or(Decimal::ZERO)
        .round_dp(6)
}

/// Net the short bucket against the long bucket, category by category.
///
/// IBKR reports the two sides separately and weights them independently, each
/// summing to 1.0, so the buckets cannot simply be concatenated. Short `nav`
/// values are already negative and short ids reuse the long id space, which
/// makes a keyed sum the correct combination: it yields the same net total as
/// the account's reported NAV.
fn net_categories(dimension: &AllocationDimension) -> Vec<(String, String, Decimal)> {
    let mut order: Vec<String> = Vec::new();
    let mut names: HashMap<String, String> = HashMap::new();
    let mut totals: HashMap<String, Decimal> = HashMap::new();

    let buckets = [
        dimension.long_positions.as_ref(),
        dimension.short_positions.as_ref(),
    ];

    for bucket in buckets.into_iter().flatten() {
        for item in &bucket.items {
            let Some(id) = item.id.as_ref().filter(|id| !id.is_empty()) else {
                continue;
            };

            if !totals.contains_key(id) {
                order.push(id.clone());
            }
            *totals.entry(id.clone()).or_insert(Decimal::ZERO) += to_decimal(item.nav);

            if let Some(name) = item.name.as_ref().filter(|n| !n.is_empty()) {
                names.entry(id.clone()).or_insert_with(|| name.clone());
            }
        }
    }

    order
        .into_iter()
        .map(|id| {
            let value = totals.get(&id).copied().unwrap_or(Decimal::ZERO);
            let name = names.get(&id).cloned().unwrap_or_else(|| id.clone());
            (id, name, value)
        })
        .collect()
}

/// Build one allocation card from an IBKR dimension.
///
/// Non-positive categories are dropped to match both the local allocation
/// service and the donut chart, which already filter on `value > 0`. A levered
/// account carries a negative cash category that would otherwise render as an
/// impossible slice. Percentages are therefore recomputed over the retained
/// positive values so each card's legend still sums to 100%.
fn build_taxonomy(
    taxonomy_id: &str,
    taxonomy_name: &str,
    color: &str,
    dimension: Option<&AllocationDimension>,
) -> TaxonomyAllocation {
    let Some(dimension) = dimension else {
        return TaxonomyAllocation::empty(taxonomy_id, taxonomy_name, color);
    };

    let positives: Vec<(String, String, Decimal)> = net_categories(dimension)
        .into_iter()
        .filter(|(_, _, value)| *value > Decimal::ZERO)
        .collect();

    let total: Decimal = positives.iter().map(|(_, _, value)| *value).sum();

    let mut categories: Vec<CategoryAllocation> = positives
        .into_iter()
        .enumerate()
        .map(|(index, (id, name, value))| {
            let percentage = if total > Decimal::ZERO {
                (value / total * dec!(100)).round_dp(2)
            } else {
                Decimal::ZERO
            };

            CategoryAllocation {
                category_id: id,
                category_name: name,
                color: CATEGORY_PALETTE[index % CATEGORY_PALETTE.len()].to_string(),
                value,
                percentage,
                children: Vec::new(),
            }
        })
        .collect();

    categories.sort_by(|a, b| b.value.cmp(&a.value));

    TaxonomyAllocation {
        taxonomy_id: taxonomy_id.to_string(),
        taxonomy_name: taxonomy_name.to_string(),
        color: color.to_string(),
        categories,
    }
}

/// Net asset value, taken as long + short on a single dimension.
///
/// Every dimension nets to the same figure, but their *gross* long totals do
/// not (IBKR nets shorts into the long total for ASSET_CLASS and
/// FINANCIAL_INSTRUMENT only). Picking a dimension at random from a hash map
/// would therefore return a gross total whenever that dimension happens to
/// carry no shorts, so the order is fixed and dimensions reporting both sides
/// are preferred.
fn net_asset_value(response: &AllocationResponse) -> Decimal {
    const PRIORITY: [&str; 5] = [
        "ASSET_CLASS",
        "FINANCIAL_INSTRUMENT",
        "SECTOR",
        "REGION",
        "COUNTRY",
    ];

    let total_of = |bucket: Option<&AllocationBucket>| {
        bucket
            .and_then(|bucket| bucket.total.as_ref())
            .map(|total| to_decimal(total.nav))
    };

    let mut fallback: Option<Decimal> = None;

    for key in PRIORITY {
        let Some(dimension) = response.allocations.get(key) else {
            continue;
        };
        let Some(long) = total_of(dimension.long_positions.as_ref()) else {
            continue;
        };

        match total_of(dimension.short_positions.as_ref()) {
            Some(short) => return long + short,
            // No short bucket: correct only if the account is genuinely
            // long-only, so keep looking for a dimension that reports both.
            None => fallback.get_or_insert(long),
        };
    }

    fallback.unwrap_or(Decimal::ZERO)
}

pub fn map_allocations(response: &AllocationResponse) -> PortfolioAllocations {
    let dimension = |key: &str| response.allocations.get(key);

    let country = build_taxonomy(
        "countries",
        "Countries",
        COUNTRY_COLOR,
        dimension("COUNTRY"),
    );

    PortfolioAllocations {
        asset_classes: build_taxonomy(
            "asset_classes",
            "Asset Classes",
            ASSET_CLASS_COLOR,
            dimension("ASSET_CLASS"),
        ),
        sectors: build_taxonomy("industries_gics", "Sectors", SECTOR_COLOR, dimension("SECTOR")),
        regions: build_taxonomy("regions", "Regions", REGION_COLOR, dimension("REGION")),
        // IBKR exposes no risk dimension; leave the card empty rather than
        // inventing a classification. The page hides it when it has no data.
        risk_category: TaxonomyAllocation::empty("risk_category", "Risk Category", "#d14d41"),
        security_types: build_taxonomy(
            "instrument_type",
            "Instrument Type",
            INSTRUMENT_COLOR,
            dimension("FINANCIAL_INSTRUMENT"),
        ),
        custom_groups: if country.categories.is_empty() {
            Vec::new()
        } else {
            vec![country]
        },
        total_value: net_asset_value(response),
    }
}

/// Fetch every allocation dimension in a single call.
///
/// `currency` is deliberately omitted: requesting any non-base currency makes
/// IBKR silently answer with the prior trading day instead of live data.
pub async fn fetch_allocations(mcp: &McpClient) -> Result<PortfolioAllocations> {
    let payload = mcp
        .call_tool("get_pa_allocation", json!({ "type": "ALL" }))
        .await?;
    let response: AllocationResponse = serde_json::from_value(payload).map_err(|err| {
        Error::Unexpected(format!(
            "IBKR returned an unexpected allocation payload: {err}"
        ))
    })?;

    Ok(map_allocations(&response))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the real `get_pa_allocation` response, including the levered
    /// cash position that makes the net total differ sharply from the long one.
    fn sample() -> AllocationResponse {
        serde_json::from_value(json!({
            "currency": "USD",
            "realtime": true,
            "allocations": {
                "ASSET_CLASS": {
                    "long_positions": {
                        "total": { "nav": 4131.301417110344, "weight": 1 },
                        "items": [
                            { "id": "CO", "name": "Commodities", "nav": 123.11, "weight": 0.0298 },
                            { "id": "EQ", "name": "Equities", "nav": 4003.83, "weight": 0.9691 },
                            { "id": "CA", "name": "Cash", "nav": 4.36, "weight": 0.0011 }
                        ]
                    },
                    "short_positions": {
                        "total": { "nav": -2124.4783245641656, "weight": 1 },
                        "items": [
                            { "id": "EQ", "name": "Equities", "nav": -94.85, "weight": 0.0446 },
                            { "id": "CA", "name": "Cash", "nav": -2029.63, "weight": 0.9554 }
                        ]
                    }
                },
                "REGION": {
                    "long_positions": {
                        "total": { "nav": 4415.75, "weight": 1 },
                        "items": [
                            { "id": "EUROPE", "name": "Europe", "nav": 2376.83, "weight": 0.5383 },
                            { "id": "NORTHA", "name": "North America", "nav": 1754.65, "weight": 0.3974 }
                        ]
                    },
                    "short_positions": {
                        "total": { "nav": -2408.92, "weight": 1 },
                        "items": [
                            { "id": "EUROPE", "name": "Europe", "nav": -2369.89, "weight": 0.9838 }
                        ]
                    }
                },
                "COUNTRY": {
                    "long_positions": {
                        "total": { "nav": 4415.75, "weight": 1 },
                        "items": [
                            { "id": "US", "name": "United States", "nav": 1754.65, "weight": 0.3974 }
                        ]
                    }
                }
            }
        }))
        .expect("sample payload should deserialize")
    }

    #[test]
    fn nets_shorts_against_longs_by_category() {
        let allocations = map_allocations(&sample());
        let categories = &allocations.asset_classes.categories;

        // Cash nets to -2025.27 and must not surface as a slice.
        assert_eq!(categories.len(), 2);

        let equities = &categories[0];
        assert_eq!(equities.category_name, "Equities");
        assert_eq!(equities.value, dec!(3908.98));

        let commodities = &categories[1];
        assert_eq!(commodities.category_name, "Commodities");
        assert_eq!(commodities.value, dec!(123.11));
    }

    #[test]
    fn percentages_sum_to_one_hundred() {
        let allocations = map_allocations(&sample());

        for taxonomy in [&allocations.asset_classes, &allocations.regions] {
            let sum: Decimal = taxonomy.categories.iter().map(|c| c.percentage).sum();
            assert!(
                (sum - dec!(100)).abs() <= dec!(0.05),
                "{} percentages summed to {sum}",
                taxonomy.taxonomy_id
            );
        }
    }

    #[test]
    fn total_value_is_net_asset_value() {
        let allocations = map_allocations(&sample());
        // 4131.301417 - 2124.478325, matching the account's reported NAV.
        assert_eq!(allocations.total_value.round_dp(2), dec!(2006.82));
    }

    /// Regression: dimensions live in a hash map, and the ones lacking a short
    /// bucket report a gross long total that is far larger than the NAV.
    /// Reading an arbitrary dimension therefore inflated the portfolio value.
    #[test]
    fn ignores_short_less_dimensions_when_netting() {
        let response: AllocationResponse = serde_json::from_value(json!({
            "allocations": {
                "COUNTRY": {
                    "long_positions": { "total": { "nav": 4415.75 }, "items": [] }
                },
                "ASSET_CLASS": {
                    "long_positions": { "total": { "nav": 4131.30 }, "items": [] },
                    "short_positions": { "total": { "nav": -2124.48 }, "items": [] }
                }
            }
        }))
        .expect("payload should deserialize");

        assert_eq!(net_asset_value(&response).round_dp(2), dec!(2006.82));
    }

    #[test]
    fn long_only_account_keeps_its_gross_total() {
        let response: AllocationResponse = serde_json::from_value(json!({
            "allocations": {
                "ASSET_CLASS": {
                    "long_positions": { "total": { "nav": 1500.0 }, "items": [] }
                }
            }
        }))
        .expect("payload should deserialize");

        assert_eq!(net_asset_value(&response).round_dp(2), dec!(1500));
    }

    #[test]
    fn missing_dimensions_yield_empty_cards() {
        let allocations = map_allocations(&sample());
        // IBKR has no risk dimension, and SECTOR was absent from this payload.
        assert!(allocations.risk_category.categories.is_empty());
        assert!(allocations.sectors.categories.is_empty());
    }

    #[test]
    fn country_dimension_becomes_a_custom_group() {
        let allocations = map_allocations(&sample());
        assert_eq!(allocations.custom_groups.len(), 1);
        assert_eq!(allocations.custom_groups[0].taxonomy_id, "countries");
    }

    #[test]
    fn handles_a_dimension_without_shorts() {
        let allocations = map_allocations(&sample());
        let country = &allocations.custom_groups[0];
        assert_eq!(country.categories.len(), 1);
        assert_eq!(country.categories[0].percentage, dec!(100));
    }
}
