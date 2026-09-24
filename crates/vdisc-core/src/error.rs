use std::{error::Error, fmt, io, path::PathBuf};

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
            | Self::MetadataRead { .. } => None,
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
