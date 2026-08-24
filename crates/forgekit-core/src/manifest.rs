use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::id::Oid;

/// CAS tip. The only mutable coordination object per repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub seq: u64,
    pub refs: BTreeMap<String, Oid>,
    pub pack_ids: Vec<Oid>,
    pub checkpoint_ids: Vec<String>,
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            seq: 0,
            refs: BTreeMap::new(),
            pack_ids: Vec::new(),
            checkpoint_ids: Vec::new(),
        }
    }

    pub fn bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
}
