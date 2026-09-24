use std::{error::Error, fmt, io, path::PathBuf};

use uuid::Uuid;

#[derive(Debug)]
pub enum VdiscError {
    Io(io::Error),

    Serialization(serde_json::Error),

    InvalidInput(String),

    TargetNotDraft { path: PathBuf },

    LocalFileRequiresLocalSource,

    LocalFileNotFound { path: PathBuf },

    LocalFileNotRegular { path: PathBuf },

    AudioValidation { path: PathBuf, reason: String },

    MetadataRead { path: PathBuf, reason: String },

    DraftIdentityMismatch { expected: Uuid, actual: Uuid },

    DiscFull { capacity: usize },

    TrackPositionOutOfBounds { position: usize, track_count: usize },

    SourceFingerprintMissing { track_id: Uuid, path: PathBuf },

    SourceMissing { track_id: Uuid, path: PathBuf },

    SourceNotRegular { track_id: Uuid, path: PathBuf },

    SourceChanged { track_id: Uuid, path: PathBuf },
}

impl fmt::Display for VdiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(f, "I/O error: {error}")
            }

            Self::Serialization(error) => {
                write!(f, "serialization error: {error}")
            }

            Self::InvalidInput(message) => {
                write!(f, "invalid input: {message}")
            }

            Self::TargetNotDraft { path } => {
                write!(
                    f,
                    "add-track target must be a .vdraft file: {}",
                    path.display()
                )
            }

            Self::LocalFileRequiresLocalSource => {
                write!(f, "local file selection requires the Local track source")
            }

            Self::LocalFileNotFound { path } => {
                write!(f, "local file does not exist: {}", path.display())
            }

            Self::LocalFileNotRegular { path } => {
                write!(f, "local source is not a regular file: {}", path.display())
            }

            Self::AudioValidation { path, reason } => {
                write!(
                    f,
                    "audio validation failed for {}: {reason}",
                    path.display()
                )
            }

            Self::MetadataRead { path, reason } => {
                write!(f, "metadata read failed for {}: {reason}", path.display())
            }

            Self::DraftIdentityMismatch { expected, actual } => {
                write!(
                    f,
                    "draft identity mismatch: expected {expected}, found {actual}"
                )
            }

            Self::DiscFull { capacity } => {
                write!(f, "CD is full: maximum capacity is {capacity} tracks")
            }

            Self::TrackPositionOutOfBounds {
                position,
                track_count,
            } => {
                write!(
                    f,
                    "track position {position} is invalid for a CD containing {track_count} tracks"
                )
            }

            Self::SourceFingerprintMissing { track_id, path } => {
                write!(
                    f,
                    "track {track_id} has no source fingerprint: {}",
                    path.display()
                )
            }

            Self::SourceMissing { track_id, path } => {
                write!(
                    f,
                    "source file for track {track_id} is missing: {}",
                    path.display()
                )
            }

            Self::SourceNotRegular { track_id, path } => {
                write!(
                    f,
                    "source for track {track_id} is no longer a regular file: {}",
                    path.display()
                )
            }

            Self::SourceChanged { track_id, path } => {
                write!(
                    f,
                    "source file for track {track_id} changed after import: {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for VdiscError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),

            Self::Serialization(error) => Some(error),

            Self::InvalidInput(_)
            | Self::TargetNotDraft { .. }
            | Self::LocalFileRequiresLocalSource
            | Self::LocalFileNotFound { .. }
            | Self::LocalFileNotRegular { .. }
            | Self::AudioValidation { .. }
            | Self::MetadataRead { .. }
            | Self::DraftIdentityMismatch { .. }
            | Self::DiscFull { .. }
            | Self::TrackPositionOutOfBounds { .. }
            | Self::SourceFingerprintMissing { .. }
            | Self::SourceMissing { .. }
            | Self::SourceNotRegular { .. }
            | Self::SourceChanged { .. } => None,
        }
    }
}

impl From<io::Error> for VdiscError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for VdiscError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub type Result<T> = std::result::Result<T, VdiscError>;
