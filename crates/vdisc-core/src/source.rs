use std::path::{Path, PathBuf};

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSourceKind {
    Local,
    Spotify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSourceSelection {
    target_path: PathBuf,
    target_disc_id: Uuid,
    source: TrackSourceKind,
}

impl TrackSourceSelection {
    pub(crate) fn new(target_path: PathBuf, target_disc_id: Uuid, source: TrackSourceKind) -> Self {
        Self {
            target_path,
            target_disc_id,
            source,
        }
    }

    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub fn target_disc_id(&self) -> Uuid {
        self.target_disc_id
    }

    pub fn source(&self) -> TrackSourceKind {
        self.source
    }
}
