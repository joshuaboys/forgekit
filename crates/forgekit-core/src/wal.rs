use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::id::Oid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalKind {
    Push,
    Checkpoint,
    Approve,
    Promote,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalEntry {
    pub seq: u64,
    pub kind: WalKind,
    pub at: DateTime<Utc>,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Oid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<Oid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

impl WalEntry {
    pub fn bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
}
