use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Result, SourceFingerprint, VdiscError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiscAppearance {
    #[serde(default)]
    surface: DiscSurface,

    #[serde(default)]
    label: Option<String>,
}

impl DiscAppearance {
    pub fn surface(&self) -> &DiscSurface {
        &self.surface
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn color(&self) -> Option<&DiscColor> {
        match &self.surface {
            DiscSurface::Color(color) => Some(color),

            _ => None,
        }
    }

    pub fn image(&self) -> Option<&DiscImage> {
        match &self.surface {
            DiscSurface::Image(image) => Some(image),

            _ => None,
        }
    }

    pub fn has_surface(&self) -> bool {
        !matches!(self.surface, DiscSurface::None)
    }

    pub(crate) fn set_surface(&mut self, surface: DiscSurface) {
        self.surface = surface;
    }

    pub(crate) fn set_label(&mut self, label: Option<String>) {
        self.label = label;
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(label) = &self.label
            && label.trim().is_empty()
        {
            return Err(VdiscError::InvalidInput(
                "disc label cannot be empty".to_string(),
            ));
        }

        if let DiscSurface::Image(image) = &self.surface {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DiscSurface {
    #[default]
    None,

    Color(DiscColor),

    Image(DiscImage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscColor {
    r: u8,
    g: u8,
    b: u8,
}

impl DiscColor {
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
