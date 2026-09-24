use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum VdiscError {
    Io(io::Error),
    InvalidInput(String),
}

impl fmt::Display for VdiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidInput(message) => {
                write!(f, "invalid input: {message}")
            }
        }
    }
}

impl Error for VdiscError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidInput(_) => None,
        }
    }
}

impl From<io::Error> for VdiscError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, VdiscError>;
