use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{Result, VdiscError, load_draft};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomizationSession {
    target_path: PathBuf,
    target_disc_id: Uuid,
}

impl CustomizationSession {
    pub fn begin(target_path: impl AsRef<Path>) -> Result<Self> {
        let target_path = target_path.as_ref().to_path_buf();

        let is_draft = target_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("vdraft");

        if !is_draft {
            return Err(VdiscError::CustomizationTargetNotDraft { path: target_path });
        }

        let draft = load_draft(&target_path)?;

        Ok(Self {
            target_path,
            target_disc_id: draft.id(),
        })
    }

    pub fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub fn target_disc_id(&self) -> Uuid {
        self.target_disc_id
    }

    pub fn verify_target(&self) -> Result<()> {
        let draft = load_draft(&self.target_path)?;

        if draft.id() != self.target_disc_id {
            return Err(VdiscError::DraftIdentityMismatch {
                expected: self.target_disc_id,

                actual: draft.id(),
            });
        }

        Ok(())
    }
}
