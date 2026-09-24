use std::{fs, io::ErrorKind, path::Path};

use crate::{DraftTrack, Result, SourceFingerprint, VdiscError, load_draft};

pub fn verify_draft_source_integrity(draft_path: impl AsRef<Path>) -> Result<()> {
    let draft = load_draft(draft_path)?;

    for track in draft.tracks() {
        verify_track(track)?;
    }

    Ok(())
}

fn verify_track(track: &DraftTrack) -> Result<()> {
    let path = track.source_path();

    let expected =
        track
            .source_fingerprint()
            .ok_or_else(|| VdiscError::SourceFingerprintMissing {
                track_id: track.id(),
                path: path.to_path_buf(),
            })?;

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,

        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(VdiscError::SourceMissing {
                track_id: track.id(),
                path: path.to_path_buf(),
            });
        }

        Err(error) => {
            return Err(error.into());
        }
    };

    if !metadata.is_file() {
        return Err(VdiscError::SourceNotRegular {
            track_id: track.id(),
            path: path.to_path_buf(),
        });
    }

    /*
     * Cheap rejection before computing SHA-256.
     */
    if metadata.len() != expected.size_bytes() {
        return Err(VdiscError::SourceChanged {
            track_id: track.id(),
            path: path.to_path_buf(),
        });
    }

    let actual = SourceFingerprint::from_file(path)?;

    if &actual != expected {
        return Err(VdiscError::SourceChanged {
            track_id: track.id(),
            path: path.to_path_buf(),
        });
    }

    Ok(())
}
