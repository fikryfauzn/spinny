use std::{fmt, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatErrorKind {
    Io,
    UnsupportedVersion,
    InvalidJson,
    InvalidSchema,
    UnsafePath,
    DuplicateEntry,
    MissingEntry,
    UnexpectedEntry,
    UnsupportedZipFeature,
    MalformedArchive,
    ResourceLimit,
    IntegrityMismatch,
    InvalidArtwork,
    UnsupportedMedia,
}

#[derive(Debug)]
pub struct FormatError {
    pub kind: FormatErrorKind,
    pub entry: Option<String>,
    pub message: String,
}

impl FormatError {
    pub(crate) fn new(kind: FormatErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            entry: None,
            message: message.into(),
        }
    }
    pub(crate) fn at(mut self, entry: &str) -> Self {
        self.entry = Some(entry.to_owned());
        self
    }
}
impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.entry {
            Some(path) => write!(f, "{:?} in {path}: {}", self.kind, self.message),
            None => write!(f, "{:?}: {}", self.kind, self.message),
        }
    }
}
impl std::error::Error for FormatError {}
impl From<io::Error> for FormatError {
    fn from(error: io::Error) -> Self {
        Self::new(FormatErrorKind::Io, error.to_string())
    }
}
pub type FormatResult<T> = Result<T, FormatError>;
