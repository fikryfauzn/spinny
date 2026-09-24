use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{Result, TrackSourceKind, TrackSourceSelection, VdiscError, load_draft};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTrackRequest {
    target_path: PathBuf,
    target_disc_id: Uuid,
}

impl AddTrackRequest {
    pub fn begin(target_path: impl AsRef<Path>) -> Result<Self> {
        let target_path = target_path.as_ref();

        validate_draft_path(target_path)?;

        let draft = load_draft(target_path)?;

        Ok(Self {
            target_path: target_path.to_path_buf(),
            target_disc_id: draft.id(),
        })
    }

    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub fn target_disc_id(&self) -> Uuid {
        self.target_disc_id
    }

    pub fn select_source(self, source: TrackSourceKind) -> TrackSourceSelection {
        TrackSourceSelection::new(self.target_path, self.target_disc_id, source)
    }
}

fn validate_draft_path(path: &Path) -> Result<()> {
    if path.extension() != Some(OsStr::new("vdraft")) {
        return Err(VdiscError::TargetNotDraft {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}
