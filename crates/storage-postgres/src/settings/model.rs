use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::wf_app_settings)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingDB {
    pub setting_key: String,
    pub setting_value: String,
}