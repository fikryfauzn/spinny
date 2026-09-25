use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{DraftDisc, Result, VdiscError};

pub fn save_draft(path: impl AsRef<Path>, draft: &DraftDisc) -> Result<()> {
    let path = path.as_ref();

    draft.validate()?;

    let data = serde_json::to_vec_pretty(draft)?;

    let temp_path = temporary_path(path)?;

    fs::write(&temp_path, data)?;

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error.into());
    }

    Ok(())
}

pub fn load_draft(path: impl AsRef<Path>) -> Result<DraftDisc> {
    let data = fs::read(path)?;

    let draft = crate::draft_migration::decode_draft(&data)?;

    draft.validate()?;

    Ok(draft)
}

fn temporary_path(path: &Path) -> Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        VdiscError::InvalidInput("draft path must contain a file name".to_string())
    })?;

    let temp_name = format!(".{}.{}.tmp", file_name.to_string_lossy(), Uuid::new_v4());

    Ok(path.with_file_name(temp_name))
}
