use std::path::{Path, PathBuf};

use crate::format::{self, FormatResult, Integrity, Manifest, ValidatedFormat};

/// Read-only domain representation of one successful VDISC validation pass.
///
/// `BurnedDisc` intentionally exposes no draft mutation API. It is metadata
/// inspection only: Objective 15 will add the supported payload reader and
/// keep an archive handle open for stable payload access.
///
/// This value does not pin the filesystem path. `verify_current_path()`
/// performs a fresh validation when callers need to detect later corruption.
#[derive(Debug, Clone)]
pub struct BurnedDisc {
    path: PathBuf,
    validated: ValidatedFormat,
}

impl BurnedDisc {
    pub fn inspect(path: impl AsRef<Path>) -> FormatResult<Self> {
        let path = path.as_ref().to_path_buf();
        let validated = format::validate_vdisc(&path)?;

        Ok(Self { path, validated })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disc_id(&self) -> &str {
        &self.validated.manifest().disc_id
    }

    pub fn title(&self) -> &str {
        &self.validated.manifest().title
    }

    pub fn track_count(&self) -> usize {
        self.validated.manifest().tracks.len()
    }

    pub fn manifest(&self) -> &Manifest {
        self.validated.manifest()
    }

    pub fn integrity(&self) -> &Integrity {
        self.validated.integrity()
    }

    pub fn verify_current_path(&self) -> FormatResult<()> {
        format::validate_vdisc(&self.path).map(|_| ())
    }
}
