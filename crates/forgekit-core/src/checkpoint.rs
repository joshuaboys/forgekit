use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{content_id, Oid};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    #[default]
    Work,
    Stable,
    Release,
}

impl CheckpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Stable => "stable",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Session {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    pub id: String,
    pub commit: Oid,
    pub kind: CheckpointKind,
    pub status: CheckpointStatus,
    pub actor: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<Session>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    pub trailer: String,
}

impl Checkpoint {
    pub fn new(
        commit: Oid,
        kind: CheckpointKind,
        actor: impl Into<String>,
        sessions: Vec<Session>,
        files: Vec<String>,
    ) -> Self {
        let actor = actor.into();
        let created_at = Utc::now();
        let mut seed = Self {
            id: String::new(),
            commit,
            kind,
            status: CheckpointStatus::Pending,
            actor,
            created_at,
            approved_by: None,
            approved_at: None,
            sessions,
            files,
            trailer: String::new(),
        };
        let id: String = content_id(&seed.bytes().unwrap_or_default())
            .chars()
            .take(12)
            .collect();
        seed.id = id.clone();
        seed.trailer = entire_trailer(&id);
        seed
    }

    pub fn bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
}

pub fn entire_trailer(id: &str) -> String {
    format!("Entire-Checkpoint: {id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_trailer_format() {
        assert_eq!(
            entire_trailer("abcdef123456"),
            "Entire-Checkpoint: abcdef123456"
        );
    }
}
