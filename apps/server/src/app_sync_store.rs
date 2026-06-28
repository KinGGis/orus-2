use async_trait::async_trait;
use wealthfolio_core::sync::{SyncEngineStatus, SyncOutboxEvent};
use wealthfolio_device_sync::engine::ReplayEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSyncTableRowCount {
    pub table: String,
    pub rows: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSyncDataSummary {
    pub total_rows: i64,
    pub non_empty_tables: Vec<AppSyncTableRowCount>,
}

#[async_trait]
pub trait AppSyncStore: Send + Sync {
    fn get_cursor(&self) -> wealthfolio_core::Result<i64>;
    async fn set_cursor(&self, cursor: i64) -> wealthfolio_core::Result<()>;
    fn get_engine_status(&self) -> wealthfolio_core::Result<SyncEngineStatus>;
    fn needs_bootstrap(&self, device_id: &str) -> wealthfolio_core::Result<bool>;
    fn get_local_sync_data_summary(&self) -> wealthfolio_core::Result<AppSyncDataSummary>;
    async fn upsert_device_config(
        &self,
        device_id: String,
        key_version: Option<i32>,
        trust_state: String,
    ) -> wealthfolio_core::Result<()>;
    async fn mark_bootstrap_complete(
        &self,
        device_id: String,
        key_version: Option<i32>,
    ) -> wealthfolio_core::Result<()>;
    fn list_pending_outbox(&self, limit: i64) -> wealthfolio_core::Result<Vec<SyncOutboxEvent>>;
    async fn mark_outbox_sent(&self, event_ids: Vec<String>) -> wealthfolio_core::Result<()>;
    async fn schedule_outbox_retry(
        &self,
        event_ids: Vec<String>,
        backoff_seconds: i64,
        last_error: Option<String>,
        last_error_code: Option<String>,
    ) -> wealthfolio_core::Result<()>;
    async fn acquire_cycle_lock(&self) -> wealthfolio_core::Result<i64>;
    fn verify_cycle_lock(&self, expected_version: i64) -> wealthfolio_core::Result<bool>;
    async fn mark_push_completed(&self) -> wealthfolio_core::Result<()>;
    async fn apply_remote_events_lww_batch(
        &self,
        events: Vec<ReplayEvent>,
    ) -> wealthfolio_core::Result<usize>;
    async fn apply_remote_event_lww(&self, event: ReplayEvent) -> wealthfolio_core::Result<bool>;
    async fn mark_pull_completed(&self) -> wealthfolio_core::Result<()>;
    async fn mark_cycle_outcome(
        &self,
        status: String,
        duration_ms: i64,
        next_retry_at: Option<String>,
    ) -> wealthfolio_core::Result<()>;
    async fn mark_engine_error(&self, message: String) -> wealthfolio_core::Result<()>;
    async fn prune_applied_events_up_to_seq(&self, seq: i64) -> wealthfolio_core::Result<usize>;
    async fn mark_outbox_dead(
        &self,
        event_ids: Vec<String>,
        error_message: Option<String>,
        error_code: Option<String>,
    ) -> wealthfolio_core::Result<()>;
    async fn export_snapshot_sqlite_image(
        &self,
        tables: Vec<String>,
    ) -> wealthfolio_core::Result<Vec<u8>>;
    async fn restore_snapshot_tables_from_file(
        &self,
        snapshot_db_path: String,
        tables: Vec<String>,
        cursor_value: i64,
        device_id: String,
        key_version: Option<i32>,
    ) -> wealthfolio_core::Result<()>;
}

#[async_trait]
impl AppSyncStore for wealthfolio_storage_sqlite::sync::AppSyncRepository {
    fn get_cursor(&self) -> wealthfolio_core::Result<i64> {
        self.get_cursor()
    }

    async fn set_cursor(&self, cursor: i64) -> wealthfolio_core::Result<()> {
        self.set_cursor(cursor).await
    }

    fn get_engine_status(&self) -> wealthfolio_core::Result<SyncEngineStatus> {
        self.get_engine_status()
    }

    fn needs_bootstrap(&self, device_id: &str) -> wealthfolio_core::Result<bool> {
        self.needs_bootstrap(device_id)
    }

    fn get_local_sync_data_summary(&self) -> wealthfolio_core::Result<AppSyncDataSummary> {
        self.get_local_sync_data_summary().map(|summary| AppSyncDataSummary {
            total_rows: summary.total_rows,
            non_empty_tables: summary
                .non_empty_tables
                .into_iter()
                .map(|row| AppSyncTableRowCount {
                    table: row.table,
                    rows: row.rows,
                })
                .collect(),
        })
    }

    async fn upsert_device_config(
        &self,
        device_id: String,
        key_version: Option<i32>,
        trust_state: String,
    ) -> wealthfolio_core::Result<()> {
        self.upsert_device_config(device_id, key_version, trust_state)
            .await
    }

    async fn mark_bootstrap_complete(
        &self,
        device_id: String,
        key_version: Option<i32>,
    ) -> wealthfolio_core::Result<()> {
        self.mark_bootstrap_complete(device_id, key_version).await
    }

    fn list_pending_outbox(&self, limit: i64) -> wealthfolio_core::Result<Vec<SyncOutboxEvent>> {
        self.list_pending_outbox(limit)
    }

    async fn mark_outbox_sent(&self, event_ids: Vec<String>) -> wealthfolio_core::Result<()> {
        self.mark_outbox_sent(event_ids).await
    }

    async fn schedule_outbox_retry(
        &self,
        event_ids: Vec<String>,
        backoff_seconds: i64,
        last_error: Option<String>,
        last_error_code: Option<String>,
    ) -> wealthfolio_core::Result<()> {
        self.schedule_outbox_retry(event_ids, backoff_seconds, last_error, last_error_code)
            .await
    }

    async fn acquire_cycle_lock(&self) -> wealthfolio_core::Result<i64> {
        self.acquire_cycle_lock().await
    }

    fn verify_cycle_lock(&self, expected_version: i64) -> wealthfolio_core::Result<bool> {
        self.verify_cycle_lock(expected_version)
    }

    async fn mark_push_completed(&self) -> wealthfolio_core::Result<()> {
        self.mark_push_completed().await
    }

    async fn apply_remote_events_lww_batch(
        &self,
        events: Vec<ReplayEvent>,
    ) -> wealthfolio_core::Result<usize> {
        self.apply_remote_events_lww_batch(
            events
                .into_iter()
                .map(|event| {
                    (
                        event.entity,
                        event.entity_id,
                        event.op,
                        event.event_id,
                        event.client_timestamp,
                        event.seq,
                        event.payload,
                    )
                })
                .collect(),
        )
        .await
    }

    async fn apply_remote_event_lww(&self, event: ReplayEvent) -> wealthfolio_core::Result<bool> {
        self.apply_remote_event_lww(
            event.entity,
            event.entity_id,
            event.op,
            event.event_id,
            event.client_timestamp,
            event.seq,
            event.payload,
        )
        .await
    }

    async fn mark_pull_completed(&self) -> wealthfolio_core::Result<()> {
        self.mark_pull_completed().await
    }

    async fn mark_cycle_outcome(
        &self,
        status: String,
        duration_ms: i64,
        next_retry_at: Option<String>,
    ) -> wealthfolio_core::Result<()> {
        self.mark_cycle_outcome(status, duration_ms, next_retry_at).await
    }

    async fn mark_engine_error(&self, message: String) -> wealthfolio_core::Result<()> {
        self.mark_engine_error(message).await
    }

    async fn prune_applied_events_up_to_seq(&self, seq: i64) -> wealthfolio_core::Result<usize> {
        self.prune_applied_events_up_to_seq(seq).await
    }

    async fn mark_outbox_dead(
        &self,
        event_ids: Vec<String>,
        error_message: Option<String>,
        error_code: Option<String>,
    ) -> wealthfolio_core::Result<()> {
        self.mark_outbox_dead(event_ids, error_message, error_code).await
    }

    async fn export_snapshot_sqlite_image(
        &self,
        tables: Vec<String>,
    ) -> wealthfolio_core::Result<Vec<u8>> {
        self.export_snapshot_sqlite_image(tables).await
    }

    async fn restore_snapshot_tables_from_file(
        &self,
        snapshot_db_path: String,
        tables: Vec<String>,
        cursor_value: i64,
        device_id: String,
        key_version: Option<i32>,
    ) -> wealthfolio_core::Result<()> {
        self.restore_snapshot_tables_from_file(
            snapshot_db_path,
            tables,
            cursor_value,
            device_id,
            key_version,
        )
        .await
    }
}