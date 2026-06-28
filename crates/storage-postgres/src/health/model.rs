use chrono::{DateTime, Utc};
use diesel::prelude::*;

use wealthfolio_core::health::IssueDismissal;

#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::wf_health_issue_dismissals)]
#[diesel(primary_key(issue_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct HealthIssueDismissalDB {
    pub issue_id: String,
    pub dismissed_at: DateTime<Utc>,
    pub data_hash: String,
}

impl From<HealthIssueDismissalDB> for IssueDismissal {
    fn from(db: HealthIssueDismissalDB) -> Self {
        Self {
            issue_id: db.issue_id,
            dismissed_at: db.dismissed_at,
            data_hash: db.data_hash,
        }
    }
}

impl From<IssueDismissal> for HealthIssueDismissalDB {
    fn from(domain: IssueDismissal) -> Self {
        Self {
            issue_id: domain.issue_id,
            dismissed_at: domain.dismissed_at,
            data_hash: domain.data_hash,
        }
    }
}