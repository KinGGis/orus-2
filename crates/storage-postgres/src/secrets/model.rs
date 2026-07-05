use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_secrets)]
pub struct SecretDB {
    pub secret_key: String,
    pub secret_value: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_secrets)]
pub struct NewSecretDB {
    pub secret_key: String,
    pub secret_value: String,
}
