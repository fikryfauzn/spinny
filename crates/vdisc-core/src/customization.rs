use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{ImageFormat, ImageReader};
use uuid::Uuid;

use crate::{
    DiscColor, DiscImage, DiscImageFormat, DiscSurface, DraftDisc, Result, SourceFingerprint,
    VdiscError, load_draft, save_draft,
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

        draft.set_disc_surface(DiscSurface::Color(color));

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

        draft.set_disc_surface(DiscSurface::Image(image.clone()));

        save_draft(&self.target_path, &draft)?;

        Ok(image)
    }

    pub fn clear_disc_surface(&self) -> Result<()> {
        let mut draft = self.load_target()?;

        draft.set_disc_surface(DiscSurface::None);

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

fn inspect_disc_image(source_path: impl AsRef<Path>) -> Result<DiscImage> {
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

    /*
     * Guess from actual file contents.
     *
     * The extension is not trusted as proof that
     * this is a real image.
     */
    let reader = ImageReader::open(&source_path)
        .map_err(|error| {
            customization_image_error(&source_path, format!("could not open image: {error}"))
        })?
        .with_guessed_format()
        .map_err(|error| {
            customization_image_error(
                &source_path,
                format!("could not determine image format: {error}"),
            )
        })?;

    let format = reader.format().ok_or_else(|| {
        customization_image_error(&source_path, "image format could not be determined")
    })?;

    let format = match format {
        ImageFormat::Png => DiscImageFormat::Png,

        ImageFormat::Jpeg => DiscImageFormat::Jpeg,

        ImageFormat::WebP => DiscImageFormat::Webp,

        other => {
            return Err(customization_image_error(
                &source_path,
                format!(
                    "unsupported disc image format: {other:?}; supported formats are PNG, JPEG, and WebP"
                ),
            ));
        }
    };

    let decoded = reader.decode().map_err(|error| {
        customization_image_error(&source_path, format!("image could not be decoded: {error}"))
    })?;

    let width = decoded.width();

    let height = decoded.height();

    if width == 0 || height == 0 {
        return Err(customization_image_error(
            &source_path,
            "image dimensions must be greater than zero",
        ));
    }

    let source_fingerprint = SourceFingerprint::from_file(&source_path).map_err(|error| {
        customization_image_error(
            &source_path,
            format!("could not fingerprint image: {error}"),
        )
    })?;

    Ok(DiscImage::new(
        source_path,
        source_fingerprint,
        width,
        height,
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
