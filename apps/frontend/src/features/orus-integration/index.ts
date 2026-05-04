/**
 * Orus Integration Module
 * Provides access to existing Orus data in Supabase
 */

// Client
export { orusSupabase, SUPABASE_URL } from "./supabase-client";

// Types
export type {
  OrusPosition,
  OrusInstrument,
  OrusAssetClass,
  OrusTrade,
  OrusProfile,
  OrusCashPosition,
  OrusNav,
  OrusPortfolioSummary,
  OrusPECompany,
  OrusPEInvestment,
  OrusAccountingEntry,
  OrusAccountingPeriod,
  OrusChartOfAccounts,
  OrusRevolutAccount,
  OrusRevolutTransaction,
  OrusShareholderParticipation,
  OrusShareholderComplianceInfo,
  OrusShareholderDocument,
  OrusShareholderDocumentRequest,
  OrusShareholderWithProfile,
  OrusUserRole,
  OrusDatabase,
} from "./types";

// Hooks
export {
  // Positions
  useOrusPositions,
  useOrusLatestPositions,
  // Instruments
  useOrusInstruments,
  useOrusInstrument,
  // Trades
  useOrusTrades,
  // Cash
  useOrusCashPositions,
  useOrusLatestCash,
  // NAV
  useOrusNavHistory,
  // Portfolio
  useOrusPortfolioSummary,
  // Private Equity
  useOrusPECompanies,
  useOrusPECompany,
  useOrusPEInvestments,
  // Accounting
  useOrusAccountingEntries,
  useOrusAccountingPeriods,
  useOrusChartOfAccounts,
  // Revolut
  useOrusRevolutAccounts,
  useOrusRevolutTransactions,
  useRevolutConnection,
  useRevolutSyncLogs,
  useRevolutSync,
  // Shareholders
  useOrusShareholderParticipations,
  // Auth
  useOrusAuth,
} from "./hooks";

// Shareholder hooks (comprehensive)
export {
  useOrusAuthWithRole,
  useShareholder,
  useShareholderAdmin,
  DOCUMENT_TYPES,
} from "./shareholder-hooks";

// Private Equity hooks (comprehensive)
export {
  // Types
  type PECompany,
  type PEInvestment,
  type PEPortfolioMetrics,
  type PEPosition,
  type PESimulationParams,
  type PESimulationResult,
  // Company hooks
  usePECompanies,
  usePECompany,
  useCreatePECompany,
  useUpdatePECompany,
  useDeletePECompany,
  // Investment hooks
  usePEInvestments,
  useCreatePEInvestment,
  useUpdatePEInvestment,
  useDeletePEInvestment,
  // Metrics
  usePEPortfolioMetrics,
  usePEPositions,
  usePESimulation,
} from "./private-equity-hooks";

// Accounting hooks (comprehensive)
export {
  // Types
  type AccountingPeriod,
  type ChartOfAccount,
  type AccountingEntry,
  type AccountingReport,
  type TrialBalanceEntry,
  type AccountingSummary,
  // Period hooks
  useAccountingPeriods,
  useAccountingPeriod,
  useCreateAccountingPeriod,
  useUpdateAccountingPeriod,
  useUpdatePeriodStatus,
  // Chart of accounts
  useChartOfAccounts,
  useCreateChartOfAccount,
  // Entry hooks
  useAccountingEntries,
  useCreateAccountingEntry,
  useUpdateAccountingEntry,
  useDeleteAccountingEntry,
  useValidateEntry,
  // Report hooks
  useAccountingReports,
  // Summary hooks
  useAccountingSummary,
  useTrialBalance,
  // Main hook
  useAccounting,
} from "./accounting-hooks";

// Risk service (comprehensive)
export {
  // Types
  type PerformanceMetrics,
  type RiskIndicators,
  type PortfolioMetrics,
  type LiquidityMetrics,
  type LeverageMetrics,
  type ComplianceMetrics,
  type DebtMetrics,
  type RiskAlert,
  type RiskDashboard,
  // Service class
  RiskService,
  // Hooks
  useRiskAlerts,
  useRiskDashboard,
  usePerformanceMetrics,
  usePortfolioRiskMetrics,
} from "./risk-service";

// Reports hooks (comprehensive)
export {
  // Types
  type ReportConfiguration,
  type ReportHistory,
  type ReportType,
  type DateRangeParams,
  type ReportFilters,
  // Config hooks
  useReportConfigs,
  useCreateReportConfig,
  useUpdateReportConfig,
  useDeleteReportConfig,
  // History hooks
  useReportsHistory,
  // Generation hooks
  useGenerateReport,
  useCancelReport,
  useRefreshReportStatus,
  useToggleReportVisibility,
  // Utilities
  downloadReport,
  getReportTypeLabel,
  getReportStatusBadgeVariant,
  getReportStatusLabel,
} from "./reports-hooks";

// Admin hooks (comprehensive)
export {
  // Types
  type UserRole,
  type UserWithRole,
  type UserInvitation,
  type Permission,
  type IBKRStatus,
  type IBKRPosition,
  type MarketDataConfig,
  type RevolutStatus,
  // User hooks
  useOrusUsers,
  useUpdateUserRole,
  useDeleteUser,
  // Invitation hooks
  useInvitations,
  useSendInvitation,
  useCancelInvitation,
  useResendInvitation,
  // Permission hooks
  useRolePermissions,
  useUpdatePermission,
  usePermissions,
  useModuleVisibility,
  // IBKR hooks
  useIBKRStatus,
  useIBKRPositions,
  useSyncIBKR,
  useConnectIBKR,
  useDisconnectIBKR,
  // Market data hooks
  useMarketDataConfig,
  useUpdateMarketDataConfig,
  // Revolut admin hooks
  useRevolutStatus,
  useConnectRevolut,
  useDisconnectRevolut,
  useSyncRevolut,
  // Utilities
  ROLE_OPTIONS,
  PERMISSION_RESOURCES,
  getRoleLabel,
  getRoleBadgeVariant,
} from "./admin-hooks";
