use std::{
    fs,
    path::{Path, PathBuf},
};

use std::io::Read;
use uuid::Uuid;

use crate::{
    DiscColor, DiscImage, DiscImageFormat, DraftDisc, Result, SourceFingerprint, VdiscError,
    load_draft, save_draft,
};

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
        self.load_target()?;

        Ok(())
    }

    pub fn set_disc_color(&self, color: DiscColor) -> Result<()> {
        let mut draft = self.load_target()?;

        draft.set_disc_color(color);

        save_draft(&self.target_path, &draft)?;

        Ok(())
    }

    pub fn set_disc_image(&self, source_path: impl AsRef<Path>) -> Result<DiscImage> {
        /*
         * Validate the image first.
         *
         * Only after successful image validation do
         * we load and mutate the target draft.
         */
        let image = inspect_disc_image(source_path)?;

        let mut draft = self.load_target()?;

        draft.set_disc_image(Some(image.clone()));

        save_draft(&self.target_path, &draft)?;

        Ok(image)
    }

    /// Reset the base to white and remove artwork; preserve label and subtitle.
    pub fn clear_disc_surface(&self) -> Result<()> {
        let mut draft = self.load_target()?;

        draft.set_disc_color(DiscColor::WHITE);

        draft.set_disc_image(None);

        save_draft(&self.target_path, &draft)?;

        Ok(())
    }

    pub fn set_disc_label(&self, label: impl Into<String>) -> Result<()> {
        let label = label.into();

        if label.trim().is_empty() {
            return Err(VdiscError::InvalidInput(
                "disc label cannot be empty; use clear_disc_label() to remove it".to_string(),
            ));
        }

        let mut draft = self.load_target()?;

        draft.set_disc_label(Some(label));

        save_draft(&self.target_path, &draft)?;

        Ok(())
    }

    pub fn clear_disc_label(&self) -> Result<()> {
        let mut draft = self.load_target()?;

        draft.set_disc_label(None);

        save_draft(&self.target_path, &draft)?;

        Ok(())
    }

    /// Reset the base to white without removing artwork.
    pub fn clear_disc_color(&self) -> Result<()> {
        self.set_disc_color(DiscColor::WHITE)
    }

    pub fn clear_disc_image(&self) -> Result<()> {
        let mut draft = self.load_target()?;
        draft.set_disc_image(None);
        save_draft(&self.target_path, &draft)
    }

    pub fn set_disc_subtitle(&self, subtitle: impl Into<String>) -> Result<()> {
        let subtitle = subtitle.into();
        if subtitle.trim().is_empty() {
            return Err(VdiscError::InvalidInput(
                "disc subtitle cannot be empty; use clear_disc_subtitle() to remove it".to_string(),
            ));
        }
        let mut draft = self.load_target()?;
        draft.set_disc_subtitle(Some(subtitle));
        save_draft(&self.target_path, &draft)
    }

    pub fn clear_disc_subtitle(&self) -> Result<()> {
        let mut draft = self.load_target()?;
        draft.set_disc_subtitle(None);
        save_draft(&self.target_path, &draft)
    }

    fn load_target(&self) -> Result<DraftDisc> {
        let draft = load_draft(&self.target_path)?;

        if draft.id() != self.target_disc_id {
            return Err(VdiscError::DraftIdentityMismatch {
                expected: self.target_disc_id,

                actual: draft.id(),
            });
        }

        Ok(draft)
    }
}

pub(crate) fn inspect_disc_image(source_path: impl AsRef<Path>) -> Result<DiscImage> {
    let source_path = source_path.as_ref();

    let metadata = fs::metadata(source_path).map_err(|error| {
        customization_image_error(source_path, format!("could not access image: {error}"))
    })?;

    if !metadata.is_file() {
        return Err(customization_image_error(
            source_path,
            "image source is not a regular file",
        ));
    }

    let source_path = fs::canonicalize(source_path).map_err(|error| {
        customization_image_error(
            source_path,
            format!("could not canonicalize image path: {error}"),
        )
    })?;

    // Bound the encoded read even if the file grows after metadata inspection.
    if metadata.len() > crate::format::MAX_ARTWORK_BYTES {
        return Err(customization_image_error(
            &source_path,
            "artwork exceeds 20 MiB",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(&source_path)?
        .take(crate::format::MAX_ARTWORK_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let inspected = crate::format::inspect_artwork(&bytes)
        .map_err(|error| customization_image_error(&source_path, error.to_string()))?;
    let format = match inspected.format.as_str() {
        "png" => DiscImageFormat::Png,
        "jpeg" => DiscImageFormat::Jpeg,
        "webp" => DiscImageFormat::Webp,
        _ => {
            return Err(customization_image_error(
                &source_path,
                "unsupported artwork format",
            ));
        }
    };
    let fingerprint = SourceFingerprint::from_bytes(&bytes);
    if SourceFingerprint::from_file(&source_path)? != fingerprint {
        return Err(customization_image_error(
            &source_path,
            "image changed during validation",
        ));
    }
    Ok(DiscImage::new(
        source_path,
        fingerprint,
        inspected.width,
        inspected.height,
        format,
    ))
}

fn customization_image_error(path: &Path, reason: impl Into<String>) -> VdiscError {
    VdiscError::InvalidInput(format!(
        "invalid disc image {}: {}",
        path.display(),
        reason.into(),
    ))
}
