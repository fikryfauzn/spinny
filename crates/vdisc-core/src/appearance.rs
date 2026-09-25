use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Result, SourceFingerprint, VdiscError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DiscAppearance {
    base_color: DiscColor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<DiscImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
}

impl DiscAppearance {
    pub fn base_color(&self) -> &DiscColor {
        &self.base_color
    }

    pub fn image(&self) -> Option<&DiscImage> {
        self.image.as_ref()
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub(crate) fn set_base_color(&mut self, color: DiscColor) {
        self.base_color = color;
    }

    pub(crate) fn set_image(&mut self, image: Option<DiscImage>) {
        self.image = image;
    }

    pub(crate) fn set_label(&mut self, label: Option<String>) {
        self.label = label;
    }

    pub(crate) fn set_subtitle(&mut self, subtitle: Option<String>) {
        self.subtitle = subtitle;
    }

    pub(crate) fn validate(&self) -> Result<()> {
        for (name, value) in [("label", self.label()), ("subtitle", self.subtitle())] {
            if let Some(value) = value
                && value.trim().is_empty()
            {
                return Err(VdiscError::InvalidInput(format!(
                    "disc {name} cannot be empty"
                )));
            }
        }

        if let Some(image) = &self.image {
            if image.width == 0 || image.height == 0 {
                return Err(VdiscError::InvalidInput(
                    "disc image dimensions must be greater than zero".to_string(),
                ));
            }
            if image.source_path.as_os_str().is_empty() {
                return Err(VdiscError::InvalidInput(
                    "disc image source path cannot be empty".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscColor {
    r: u8,
    g: u8,
    b: u8,
}

impl Default for DiscColor {
    fn default() -> Self {
        Self::WHITE
    }
}

impl DiscColor {
    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn r(&self) -> u8 {
        self.r
    }

    pub fn g(&self) -> u8 {
        self.g
    }

    pub fn b(&self) -> u8 {
        self.b
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b,)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscImage {
    source_path: PathBuf,

    source_fingerprint: SourceFingerprint,

    width: u32,
    height: u32,

    format: DiscImageFormat,
}

impl DiscImage {
    pub(crate) fn new(
        source_path: PathBuf,
        source_fingerprint: SourceFingerprint,
        width: u32,
        height: u32,
        format: DiscImageFormat,
    ) -> Self {
        Self {
            source_path,
            source_fingerprint,
            width,
            height,
            format,
        }
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn source_fingerprint(&self) -> &SourceFingerprint {
        &self.source_fingerprint
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> DiscImageFormat {
        self.format
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscImageFormat {
    Png,
    Jpeg,
    Webp,
}

impl DiscImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "png",

            Self::Jpeg => "jpeg",

            Self::Webp => "webp",
        }
    }
}
