//! Database models for Revolut integration.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use wealthfolio_core::revolut::{
    NewRevolutAuthToken, RevolutAccount, RevolutAuthToken, RevolutSyncLog, RevolutTransaction,
};

/// Database model for revolut_auth_tokens table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::revolut_auth_tokens)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RevolutAuthTokenDB {
    pub id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<RevolutAuthTokenDB> for RevolutAuthToken {
    fn from(db: RevolutAuthTokenDB) -> Self {
        Self {
            id: db.id,
            access_token: db.access_token,
            refresh_token: db.refresh_token,
            token_type: db.token_type,
            scope: db.scope,
            expires_at: db.expires_at,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl From<RevolutAuthToken> for RevolutAuthTokenDB {
    fn from(domain: RevolutAuthToken) -> Self {
        Self {
            id: domain.id,
            access_token: domain.access_token,
            refresh_token: domain.refresh_token,
            token_type: domain.token_type,
            scope: domain.scope,
            expires_at: domain.expires_at,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

impl From<NewRevolutAuthToken> for RevolutAuthTokenDB {
    fn from(new_token: NewRevolutAuthToken) -> Self {
        let now = chrono::Utc::now();
        let now_str = now.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        
        // Calculate expires_at from expires_in or use provided expires_at
        let expires_at = new_token.expires_at.unwrap_or_else(|| {
            if let Some(expires_in) = new_token.expires_in {
                let expiry = now + chrono::Duration::seconds(expires_in);
                expiry.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
            } else {
                // Default to 1 hour if not specified
                let expiry = now + chrono::Duration::hours(1);
                expiry.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
            }
        });

        Self {
            id: new_token.id.unwrap_or_default(), // Will be overwritten by repository
            access_token: new_token.access_token,
            refresh_token: new_token.refresh_token,
            token_type: new_token.token_type.unwrap_or_else(|| "Bearer".to_string()),
            scope: new_token.scope,
            expires_at,
            created_at: now_str.clone(),
            updated_at: now_str,
        }
    }
}

/// Database model for revolut_accounts table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::revolut_accounts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RevolutAccountDB {
    pub id: String,
    pub revolut_account_id: String,
    pub name: String,
    pub currency: String,
    pub balance: String,
    pub state: String,
    pub wf_account_id: Option<String>,
    pub last_synced_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<RevolutAccountDB> for RevolutAccount {
    fn from(db: RevolutAccountDB) -> Self {
        Self {
            id: db.id,
            revolut_account_id: db.revolut_account_id,
            name: db.name,
            currency: db.currency,
            balance: db.balance,
            state: db.state,
            wf_account_id: db.wf_account_id,
            last_synced_at: db.last_synced_at,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

impl From<RevolutAccount> for RevolutAccountDB {
    fn from(domain: RevolutAccount) -> Self {
        Self {
            id: domain.id,
            revolut_account_id: domain.revolut_account_id,
            name: domain.name,
            currency: domain.currency,
            balance: domain.balance,
            state: domain.state,
            wf_account_id: domain.wf_account_id,
            last_synced_at: domain.last_synced_at,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

/// Database model for revolut_transactions table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::revolut_transactions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RevolutTransactionDB {
    pub id: String,
    pub revolut_transaction_id: String,
    pub revolut_account_id: String,
    #[diesel(column_name = tx_type)]
    pub tx_type: String,
    pub state: String,
    pub amount: String,
    pub currency: String,
    pub description: Option<String>,
    pub merchant_name: Option<String>,
    pub merchant_category: Option<String>,
    pub reference: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub balance_after: Option<String>,
    pub wf_activity_id: Option<String>,
    pub synced_to_activities: i32,
}

impl From<RevolutTransactionDB> for RevolutTransaction {
    fn from(db: RevolutTransactionDB) -> Self {
        Self {
            id: db.id,
            revolut_transaction_id: db.revolut_transaction_id,
            revolut_account_id: db.revolut_account_id,
            r#type: db.tx_type,
            state: db.state,
            amount: db.amount,
            currency: db.currency,
            description: db.description,
            merchant_name: db.merchant_name,
            merchant_category: db.merchant_category,
            reference: db.reference,
            completed_at: db.completed_at,
            created_at: db.created_at,
            balance_after: db.balance_after,
            wf_activity_id: db.wf_activity_id,
            synced_to_activities: db.synced_to_activities != 0,
        }
    }
}

impl From<RevolutTransaction> for RevolutTransactionDB {
    fn from(domain: RevolutTransaction) -> Self {
        Self {
            id: domain.id,
            revolut_transaction_id: domain.revolut_transaction_id,
            revolut_account_id: domain.revolut_account_id,
            tx_type: domain.r#type,
            state: domain.state,
            amount: domain.amount,
            currency: domain.currency,
            description: domain.description,
            merchant_name: domain.merchant_name,
            merchant_category: domain.merchant_category,
            reference: domain.reference,
            completed_at: domain.completed_at,
            created_at: domain.created_at,
            balance_after: domain.balance_after,
            wf_activity_id: domain.wf_activity_id,
            synced_to_activities: if domain.synced_to_activities { 1 } else { 0 },
        }
    }
}

/// Database model for revolut_sync_log table
#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::revolut_sync_log)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RevolutSyncLogDB {
    pub id: String,
    pub sync_type: String,
    pub status: String,
    pub accounts_synced: Option<i32>,
    pub transactions_synced: Option<i32>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

impl From<RevolutSyncLogDB> for RevolutSyncLog {
    fn from(db: RevolutSyncLogDB) -> Self {
        Self {
            id: db.id,
            sync_type: db.sync_type,
            status: db.status,
            accounts_synced: db.accounts_synced,
            transactions_synced: db.transactions_synced,
            error_message: db.error_message,
            started_at: db.started_at,
            completed_at: db.completed_at,
        }
    }
}

impl From<RevolutSyncLog> for RevolutSyncLogDB {
    fn from(domain: RevolutSyncLog) -> Self {
        Self {
            id: domain.id,
            sync_type: domain.sync_type,
            status: domain.status,
            accounts_synced: domain.accounts_synced,
            transactions_synced: domain.transactions_synced,
            error_message: domain.error_message,
            started_at: domain.started_at,
            completed_at: domain.completed_at,
        }
    }
}
