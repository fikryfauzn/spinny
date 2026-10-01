use std::{
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{DraftDisc, Result, VdiscError};

#[derive(Debug)]
enum DestinationState {
    Missing,
    ExistingDraft(Vec<u8>),
}

pub fn save_draft(path: impl AsRef<Path>, draft: &DraftDisc) -> Result<()> {
    let path = path.as_ref();

    ensure_draft_path(path)?;
    draft.validate()?;

    /*
     * Capture what currently owns this path before any temporary file is
     * created. A draft save may create a missing .vdraft or replace another
     * valid draft, but it must never replace a finalized/unknown object.
     */
    let destination = capture_destination(path)?;

    let data = serde_json::to_vec_pretty(draft)?;
    let temp_path = temporary_path(path)?;

    if let Err(error) = fs::write(&temp_path, data) {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }

    /*
     * Recheck immediately before publication. This is deliberately not a
     * multi-process filesystem lock, but it prevents the supported API from
     * blindly replacing a target that changed while this save was prepared.
     */
    if let Err(error) = ensure_destination_unchanged(path, &destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }

    Ok(())
}

pub fn load_draft(path: impl AsRef<Path>) -> Result<DraftDisc> {
    let path = path.as_ref();

    ensure_draft_path(path)?;

    let data = fs::read(path)?;

    let draft = crate::draft_migration::decode_draft(&data)?;

    draft.validate()?;

    Ok(draft)
}

fn ensure_draft_path(path: &Path) -> Result<()> {
    if path.extension() != Some(OsStr::new("vdraft")) {
        return Err(VdiscError::DraftTargetNotEditable {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

fn capture_destination(path: &Path) -> Result<DestinationState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            /*
             * Reject symlinks, directories, devices, sockets, and other
             * non-regular targets. A draft mutation operates on one explicit
             * editable object, not on an alias to some other filesystem node.
             */
            if !metadata.file_type().is_file() {
                return Err(not_editable(path));
            }

            let bytes = fs::read(path)?;

            let draft =
                crate::draft_migration::decode_draft(&bytes).map_err(|_| not_editable(path))?;

            draft.validate().map_err(|_| not_editable(path))?;

            Ok(DestinationState::ExistingDraft(bytes))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(DestinationState::Missing),
        Err(error) => Err(error.into()),
    }
}

fn ensure_destination_unchanged(path: &Path, expected: &DestinationState) -> Result<()> {
    match expected {
        DestinationState::Missing => match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err(VdiscError::DraftTargetChanged {
                path: path.to_path_buf(),
            }),
        },
        DestinationState::ExistingDraft(expected_bytes) => {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                if error.kind() == ErrorKind::NotFound {
                    VdiscError::DraftTargetChanged {
                        path: path.to_path_buf(),
                    }
                } else {
                    error.into()
                }
            })?;

            if !metadata.file_type().is_file() {
                return Err(VdiscError::DraftTargetChanged {
                    path: path.to_path_buf(),
                });
            }

            let current_bytes = fs::read(path)?;

            if current_bytes != *expected_bytes {
                return Err(VdiscError::DraftTargetChanged {
                    path: path.to_path_buf(),
                });
            }

            Ok(())
        }
    }
}

fn not_editable(path: &Path) -> VdiscError {
    VdiscError::DraftTargetNotEditable {
        path: path.to_path_buf(),
    }
}

fn temporary_path(path: &Path) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        VdiscError::InvalidInput("draft path must contain a file name".to_string())
    })?;

    let temp_name = format!(".{}.{}.tmp", file_name.to_string_lossy(), Uuid::new_v4());

    Ok(path.with_file_name(temp_name))
}
