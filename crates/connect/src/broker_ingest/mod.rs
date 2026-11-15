//! Broker ingest domain contracts (connect-owned API surface).

mod core_adapter;
mod models;
mod reconciliation;
mod snapshot;

pub use core_adapter::CoreImportRunRepositoryAdapter;
pub use models::{
    BrokerSyncState, BrokerSyncStateRepositoryTrait, ImportRun, ImportRunMode,
    ImportRunRepositoryTrait, ImportRunStatus, ImportRunSummary, ImportRunType,
    PlaidInvestmentsCheckpoint, PlaidSyncCheckpoint, ReviewMode, SnapTradeCheckpoint, SyncStatus,
};
pub use reconciliation::{compare_valuations, ValuationGap};
pub use snapshot::{
    BrokerSnapshotKind, BrokerSnapshotMetric, BrokerSnapshotRepositoryTrait, BrokerSnapshotSide,
    NewBrokerSnapshot, StoredBrokerSnapshot,
};
