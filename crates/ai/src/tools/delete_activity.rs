//! Delete Activity tool - remove existing activities/transactions.
//!
//! This tool enables the AI assistant to delete existing transactions.
//! The user must confirm before deletion is executed.

use log::debug;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;

// ============================================================================
// Tool Arguments (LLM Input)
// ============================================================================

/// Arguments for the delete_activity tool.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteActivityArgs {
    /// Activity ID to delete (required).
    pub activity_id: String,

    /// Reason for deletion (optional, for audit trail).
    pub reason: Option<String>,
}

// ============================================================================
// Output Types
// ============================================================================

/// Output for the delete_activity tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteActivityOutput {
    /// Whether the deletion was successful.
    pub success: bool,

    /// Summary of the deleted activity.
    pub deleted_activity: Option<DeletedActivitySummary>,

    /// Error message if failed.
    pub error: Option<String>,
}

/// Summary of the deleted activity for confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedActivitySummary {
    pub id: String,
    pub activity_type: String,
    pub activity_date: String,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub amount: Option<f64>,
    pub account_id: String,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to delete an existing activity.
pub struct DeleteActivityTool<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> DeleteActivityTool<E> {
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }
}

impl<E: AiEnvironment> Clone for DeleteActivityTool<E> {
    fn clone(&self) -> Self {
        Self {
            env: self.env.clone(),
        }
    }
}

impl<E: AiEnvironment + 'static> Tool for DeleteActivityTool<E> {
    const NAME: &'static str = "delete_activity";

    type Error = AiError;
    type Args = DeleteActivityArgs;
    type Output = DeleteActivityOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Delete an existing investment activity/transaction. Use search_activities first to find the activity_id. This action is irreversible.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "activityId": {
                        "type": "string",
                        "description": "ID of the activity to delete (from search_activities)"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for deletion (optional, for audit purposes)"
                    }
                },
                "required": ["activityId"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!("Deleting activity: {:?}", args);

        if args.activity_id.trim().is_empty() {
            return Ok(DeleteActivityOutput {
                success: false,
                deleted_activity: None,
                error: Some("Activity ID is required".to_string()),
            });
        }

        // Call the activity service to delete
        match self
            .env
            .activity_service()
            .delete_activity(args.activity_id.clone())
            .await
        {
            Ok(activity) => {
                Ok(DeleteActivityOutput {
                    success: true,
                    deleted_activity: Some(DeletedActivitySummary {
                        id: activity.id.clone(),
                        activity_type: activity.activity_type.clone(),
                        activity_date: activity.activity_date.to_string(),
                        symbol: activity.asset_id.clone(),
                        quantity: activity.quantity.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        amount: activity.amount.map(|d| d.to_string().parse().unwrap_or(0.0)),
                        account_id: activity.account_id.clone(),
                    }),
                    error: None,
                })
            }
            Err(e) => Ok(DeleteActivityOutput {
                success: false,
                deleted_activity: None,
                error: Some(format!("Delete failed: {}", e)),
            }),
        }
    }
}
