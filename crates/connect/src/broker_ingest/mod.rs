//! Broker ingest domain contracts (connect-owned API surface).

mod core_adapter;
mod models;
mod snapshot;

pub use core_adapter::CoreImportRunRepositoryAdapter;
pub use models::{
    BrokerSyncState, BrokerSyncStateRepositoryTrait, ImportRun, ImportRunMode,
    ImportRunRepositoryTrait, ImportRunStatus, ImportRunSummary, ImportRunType,
    PlaidInvestmentsCheckpoint, PlaidSyncCheckpoint, ReviewMode, SnapTradeCheckpoint, SyncStatus,
};
pub use snapshot::{
    BrokerSnapshotKind, BrokerSnapshotMetric, BrokerSnapshotRepositoryTrait, BrokerSnapshotSide,
    NewBrokerSnapshot, StoredBrokerSnapshot,
};
