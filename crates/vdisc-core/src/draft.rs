use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DISC_TRACK_CAPACITY, Result, VdiscError};

pub const DRAFT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftDisc {
    draft_version: u32,
    id: Uuid,
    title: String,
    created_at_unix: u64,
}

impl DraftDisc {
    pub fn new(title: impl Into<String>) -> Result<Self> {
        let title = title.into();

        validate_title(&title)?;

        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                VdiscError::InvalidInput(format!("system clock is before Unix epoch: {error}"))
            })?
            .as_secs();

        Ok(Self {
            draft_version: DRAFT_FORMAT_VERSION,
            id: Uuid::new_v4(),
            title,
            created_at_unix,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn created_at_unix(&self) -> u64 {
        self.created_at_unix
    }

    pub fn capacity(&self) -> usize {
        DISC_TRACK_CAPACITY
    }

    pub fn validate(&self) -> Result<()> {
        if self.draft_version != DRAFT_FORMAT_VERSION {
            return Err(VdiscError::InvalidInput(format!(
                "unsupported draft format version: {}",
                self.draft_version
            )));
        }

        validate_title(&self.title)
    }
}

fn validate_title(title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(VdiscError::InvalidInput(
            "CD title cannot be empty".to_string(),
        ));
    }

    Ok(())
}
