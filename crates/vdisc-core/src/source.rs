use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{Result, VdiscError};

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

    pub fn select_local_file(self, source_path: impl AsRef<Path>) -> Result<LocalFileSelection> {
        if self.source != TrackSourceKind::Local {
            return Err(VdiscError::LocalFileRequiresLocalSource);
        }

        let source_path = source_path.as_ref();

        let metadata = match fs::metadata(source_path) {
            Ok(metadata) => metadata,

            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(VdiscError::LocalFileNotFound {
                    path: source_path.to_path_buf(),
                });
            }

            Err(error) => {
                return Err(error.into());
            }
        };

        if !metadata.is_file() {
            return Err(VdiscError::LocalFileNotRegular {
                path: source_path.to_path_buf(),
            });
        }

        let source_path = fs::canonicalize(source_path)?;

        Ok(LocalFileSelection {
            target_path: self.target_path,
            target_disc_id: self.target_disc_id,
            source_path,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileSelection {
    target_path: PathBuf,
    target_disc_id: Uuid,
    source_path: PathBuf,
}

impl LocalFileSelection {
    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub fn target_disc_id(&self) -> Uuid {
        self.target_disc_id
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }
}
