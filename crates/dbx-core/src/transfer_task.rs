use serde::{Deserialize, Serialize};

use crate::transfer::{TransferMode, TransferTableNameCase};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferTask {
    pub id: String,
    pub name: String,
    pub source_connection_id: String,
    pub source_database: String,
    pub source_schema: String,
    pub target_connection_id: String,
    pub target_database: String,
    pub target_schema: String,
    pub tables: Vec<String>,
    pub create_table: bool,
    #[serde(default)]
    pub mode: TransferMode,
    #[serde(default)]
    pub target_table_name_case: TransferTableNameCase,
    pub batch_size: usize,
    #[serde(default)]
    pub order_index: i64,
    pub last_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
