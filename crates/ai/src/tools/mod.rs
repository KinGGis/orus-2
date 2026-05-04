//! AI assistant tools for portfolio data access.
//!
//! This module provides tools that implement rig-core's Tool trait:
//! - GetAccountsTool: Fetch active investment accounts
//! - GetHoldingsTool: Fetch portfolio holdings
//! - GetAssetAllocationTool: Calculate portfolio allocation by category
//! - GetPerformanceTool: Fetch portfolio performance metrics
//! - GetValuationHistoryTool: Fetch portfolio valuation history
//! - SearchActivitiesTool: Search transactions
//! - GetIncomeTool: Fetch income summaries (dividends, interest, other income)
//! - GetGoalsTool: Fetch investment goals with progress
//! - RecordActivityTool: Create activity drafts from natural language
//! - RecordActivitiesTool: Create multiple activity drafts from natural language
//! - UpdateActivityTool: Modify existing activities
//! - DeleteActivityTool: Remove activities
//! - RiskAnalysisTool: Analyze portfolio risk and concentration
//! - PortfolioSummaryTool: Get comprehensive portfolio overview
//!
//! All tools are designed to work with the AiEnvironment trait for dependency injection.

pub mod accounts;
pub mod activities;
pub mod allocation;
pub mod constants;
pub mod delete_activity;
pub mod goals;
pub mod holdings;
pub mod import_csv;
pub mod income;
pub mod performance;
pub mod portfolio_summary;
pub mod read_file;
pub mod record_activities;
pub mod record_activity;
pub mod risk_analysis;
pub mod update_activity;
pub mod valuation;

// Re-export constants
pub use constants::*;

// Re-export tools
pub use accounts::GetAccountsTool;
pub use activities::SearchActivitiesTool;
pub use allocation::GetAssetAllocationTool;
pub use delete_activity::DeleteActivityTool;
pub use goals::GetGoalsTool;
pub use holdings::GetHoldingsTool;
pub use import_csv::ImportCsvTool;
pub use income::GetIncomeTool;
pub use performance::GetPerformanceTool;
pub use portfolio_summary::PortfolioSummaryTool;
pub use read_file::ReadFileTool;
pub use record_activities::RecordActivitiesTool;
pub use record_activity::RecordActivityTool;
pub use risk_analysis::RiskAnalysisTool;
pub use update_activity::UpdateActivityTool;
pub use valuation::GetValuationHistoryTool;

use std::sync::Arc;

use crate::env::AiEnvironment;

/// Container for all AI tools, simplifying tool registration across providers.
pub struct ToolSet<E: AiEnvironment> {
    pub holdings: GetHoldingsTool<E>,
    pub allocation: GetAssetAllocationTool<E>,
    pub accounts: GetAccountsTool<E>,
    pub activities: SearchActivitiesTool<E>,
    pub income: GetIncomeTool<E>,
    pub valuation: GetValuationHistoryTool<E>,
    pub goals: GetGoalsTool<E>,
    pub performance: GetPerformanceTool<E>,
    pub record_activity: RecordActivityTool<E>,
    pub record_activities: RecordActivitiesTool<E>,
    pub import_csv: ImportCsvTool<E>,
    pub update_activity: UpdateActivityTool<E>,
    pub delete_activity: DeleteActivityTool<E>,
    pub risk_analysis: RiskAnalysisTool<E>,
    pub portfolio_summary: PortfolioSummaryTool<E>,
    pub read_file: ReadFileTool,
}

impl<E: AiEnvironment> ToolSet<E> {
    /// Create a new tool set with all portfolio tools.
    pub fn new(env: Arc<E>, base_currency: String) -> Self {
        Self {
            holdings: GetHoldingsTool::new(env.clone(), base_currency.clone()),
            allocation: GetAssetAllocationTool::new(env.clone(), base_currency.clone()),
            accounts: GetAccountsTool::new(env.clone()),
            activities: SearchActivitiesTool::new(env.clone()),
            income: GetIncomeTool::new(env.clone()),
            valuation: GetValuationHistoryTool::new(env.clone(), base_currency.clone()),
            goals: GetGoalsTool::new(env.clone()),
            performance: GetPerformanceTool::new(env.clone(), base_currency.clone()),
            record_activity: RecordActivityTool::new(env.clone()),
            record_activities: RecordActivitiesTool::new(env.clone()),
            import_csv: ImportCsvTool::new(env.clone(), base_currency.clone()),
            update_activity: UpdateActivityTool::new(env.clone()),
            delete_activity: DeleteActivityTool::new(env.clone()),
            risk_analysis: RiskAnalysisTool::new(env.clone(), base_currency.clone()),
            portfolio_summary: PortfolioSummaryTool::new(env, base_currency),
            read_file: ReadFileTool::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::test_env::MockEnvironment;

    #[test]
    fn test_tool_set_creation() {
        let env = Arc::new(MockEnvironment::new());
        let _tools = ToolSet::new(env, "USD".to_string());
    }
}
