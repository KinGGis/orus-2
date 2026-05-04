//! Update Activity tool - modify existing activities/transactions.
//!
//! This tool enables the AI assistant to update existing transactions.
//! Users can modify quantity, price, date, fees, or other attributes.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use wealthfolio_core::activities::{ActivityUpdate as CoreActivityUpdate, SymbolInput};

// ============================================================================
// Tool Arguments (LLM Input)
// ============================================================================

/// Arguments for the update_activity tool.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateActivityArgs {
    /// Activity ID to update (required).
    pub activity_id: String,

    /// Account ID (required).
    pub account_id: String,

    /// Activity type (required): BUY, SELL, DIVIDEND, DEPOSIT, WITHDRAWAL, etc.
    pub activity_type: String,

    /// ISO 8601 date (YYYY-MM-DD). Required.
    pub activity_date: String,

    /// Symbol for trading activities (optional for cash activities).
    pub symbol: Option<String>,

    /// Updated quantity (optional, only if changed).
    pub quantity: Option<f64>,

    /// Updated unit price (optional, only if changed).
    pub unit_price: Option<f64>,

    /// Updated amount (optional, for DEPOSIT/WITHDRAWAL/DIVIDEND).
    pub amount: Option<f64>,

    /// Updated fee (optional).
    pub fee: Option<f64>,

    /// Currency code (required).
    pub currency: String,

    /// Updated notes (optional).
    pub notes: Option<String>,
}

// ============================================================================
// Output Types
// ============================================================================

/// Output for the update_activity tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateActivityOutput {
    /// Whether the update was successful.
    pub success: bool,

    /// The updated activity details.
    pub activity: Option<ActivitySummary>,

    /// Error message if failed.
    pub error: Option<String>,
}

/// Summary of the updated activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    pub id: String,
    pub activity_type: String,
    pub activity_date: String,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub amount: Option<f64>,
    pub fee: Option<f64>,
    pub currency: String,
    pub account_id: String,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to update an existing activity.
pub struct UpdateActivityTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> UpdateActivityTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for UpdateActivityTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for UpdateActivityTool<E> {
    const NAME: &'static str = "update_activity";

    type Error = AiError;
    type Args = UpdateActivityArgs;
    type Output = UpdateActivityOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Update an existing investment activity/transaction. Modify quantity, price, date, fees, or other attributes. Use search_activities first to find the activity_id.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "activityId": {
                        "type": "string",
                        "description": "ID of the activity to update (from search_activities)"
                    },
                    "accountId": {
                        "type": "string",
                        "description": "Account ID the activity belongs to"
                    },
                    "activityType": {
                        "type": "string",
                        "description": "Activity type",
                        "enum": ["BUY", "SELL", "DIVIDEND", "DEPOSIT", "WITHDRAWAL", "TRANSFER_IN", "TRANSFER_OUT", "INTEREST", "FEE", "SPLIT", "TAX"]
                    },
                    "activityDate": {
                        "type": "string",
                        "description": "Activity date in YYYY-MM-DD format"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Symbol/ticker for trading activities"
                    },
                    "quantity": {
                        "type": "number",
                        "description": "Updated quantity (shares/units)"
                    },
                    "unitPrice": {
                        "type": "number",
                        "description": "Updated price per unit"
                    },
                    "amount": {
                        "type": "number",
                        "description": "Updated total amount"
                    },
                    "fee": {
                        "type": "number",
                        "description": "Updated transaction fee"
                    },
                    "currency": {
                        "type": "string",
                        "description": "Currency code (e.g., USD, EUR)"
                    },
                    "notes": {
                        "type": "string",
                        "description": "Updated notes"
                    }
                },
                "required": ["activityId", "accountId", "activityType", "activityDate", "currency"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!("Updating activity: {:?}", args);

        // Convert f64 to Decimal using proper conversion
        let quantity = args.quantity.and_then(|v| Decimal::from_f64_retain(v));
        let unit_price = args.unit_price.and_then(|v| Decimal::from_f64_retain(v));
        let amount = args.amount.and_then(|v| Decimal::from_f64_retain(v));
        let fee = args.fee.and_then(|v| Decimal::from_f64_retain(v));

        // Build the symbol input if symbol provided
        let symbol_input = args.symbol.map(|s| SymbolInput {
            id: None,
            symbol: Some(s),
            exchange_mic: None,
            kind: None,
            name: None,
            quote_mode: None,
            quote_ccy: None,
            instrument_type: None,
        });

        // Build the update request
        let update = CoreActivityUpdate {
            id: args.activity_id.clone(),
            account_id: args.account_id.clone(),
            symbol: symbol_input,
            activity_type: args.activity_type.clone(),
            subtype: None,
            activity_date: args.activity_date.clone(),
            quantity: quantity.map(Some),
            unit_price: unit_price.map(Some),
            currency: args.currency.clone(),
            fee: fee.map(Some),
            amount: amount.map(Some),
            status: None,
            notes: args.notes.clone(),
            fx_rate: None,
            metadata: None,
        };

        // Validate the update
        if let Err(e) = update.validate() {
            return Ok(UpdateActivityOutput {
                success: false,
                activity: None,
                error: Some(format!("Validation error: {}", e)),
            });
        }

        // Call the activity service to update
        match self.env.activity_service().update_activity(update).await {
            Ok(activity) => {
                Ok(UpdateActivityOutput {
                    success: true,
                    activity: Some(ActivitySummary {
                        id: activity.id.clone(),
                        activity_type: activity.activity_type.clone(),
                        activity_date: activity.activity_date.to_string(),
                        symbol: activity.asset_id.clone(),
                        quantity: activity.quantity.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        unit_price: activity.unit_price.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        amount: activity.amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        fee: activity.fee.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        currency: activity.currency.clone(),
                        account_id: activity.account_id.clone(),
                    }),
                    error: None,
                })
            }
            Err(e) => {
                Ok(UpdateActivityOutput {
                    success: false,
                    activity: None,
                    error: Some(format!("Update failed: {}", e)),
                })
            }
        }
    }
}
