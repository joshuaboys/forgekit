//! Optional satellite (GitHub) outcome attached to a promote.

use serde::{Deserialize, Serialize};

/// Result of attempting to push promote evidence to a satellite remote.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SatelliteResult {
    /// True when the remote accepted the promote evidence.
    pub pushed: bool,
    /// Remote ref updated, if any (e.g. refs/heads/forgekit/promotes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ref: Option<String>,
    /// Browser or API URL for the evidence, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Error message when push was attempted and failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SatelliteResult {
    pub fn recorded_only() -> Self {
        Self {
            pushed: false,
            remote_ref: None,
            url: None,
            error: None,
        }
    }

    pub fn ok(remote_ref: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            pushed: true,
            remote_ref: Some(remote_ref.into()),
            url: Some(url.into()),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            pushed: false,
            remote_ref: None,
            url: None,
            error: Some(msg.into()),
        }
    }
}
