use crate::PreflightReport;
use std::{error::Error, fmt, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnPhase {
    Load,
    Preflight,
    Clock,
    Source,
    Write,
    FileSync,
    Verify,
    Publish,
}
#[derive(Debug)]
pub struct CleanupFailure {
    pub temporary_path: PathBuf,
    pub diagnostic: String,
}
#[derive(Debug)]
pub struct BurnError {
    pub phase: BurnPhase,
    pub path: PathBuf,
    pub diagnostic: String,
    pub preflight: Option<Box<PreflightReport>>,
    pub cleanup: Option<Box<CleanupFailure>>,
}
impl BurnError {
    pub(crate) fn new(
        phase: BurnPhase,
        path: impl Into<PathBuf>,
        diagnostic: impl ToString,
    ) -> Self {
        Self {
            phase,
            path: path.into(),
            diagnostic: diagnostic.to_string(),
            preflight: None,
            cleanup: None,
        }
    }
}
impl fmt::Display for BurnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "burn {:?} at {}: {}",
            self.phase,
            self.path.display(),
            self.diagnostic
        )?;
        if let Some(cleanup) = &self.cleanup {
            write!(
                f,
                "; temporary cleanup failed at {}: {}",
                cleanup.temporary_path.display(),
                cleanup.diagnostic
            )?;
        }
        Ok(())
    }
}
impl Error for BurnError {}
