//! Version-aware loading. Migration does not write to the source file.
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    DRAFT_FORMAT_VERSION, DiscAppearance, DiscColor, DiscImage, DraftDisc, DraftTrack, Result,
    VdiscError,
};

#[derive(Deserialize)]
struct VersionHeader {
    draft_version: u32,
}

pub(crate) fn decode_draft(data: &[u8]) -> Result<DraftDisc> {
    let header: VersionHeader = serde_json::from_slice(data)?;
    match header.draft_version {
        1 => {
            let legacy: LegacyDraft = serde_json::from_slice(data)?;
            legacy.into_current()
        }
        DRAFT_FORMAT_VERSION => Ok(serde_json::from_slice(data)?),
        version => Err(VdiscError::InvalidInput(format!(
            "unsupported draft format version: {version}"
        ))),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyDraft {
    draft_version: u32,
    id: Uuid,
    title: String,
    created_at_unix: u64,
    #[serde(default)]
    tracks: Vec<DraftTrack>,
    #[serde(default)]
    appearance: LegacyAppearance,
}

impl LegacyDraft {
    fn into_current(self) -> Result<DraftDisc> {
        if self.draft_version != 1 {
            return Err(VdiscError::InvalidInput(
                "expected a version 1 draft".to_string(),
            ));
        }
        let mut appearance = DiscAppearance::default();
        match self.appearance.surface {
            LegacySurface::None => {}
            LegacySurface::Color(color) => appearance.set_base_color(color),
            LegacySurface::Image(image) => appearance.set_image(Some(image)),
        }
        appearance.set_label(self.appearance.label);
        DraftDisc::from_legacy(
            self.id,
            self.title,
            self.created_at_unix,
            self.tracks,
            appearance,
        )
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct LegacyAppearance {
    #[serde(default)]
    surface: LegacySurface,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum LegacySurface {
    #[default]
    None,
    Color(DiscColor),
    Image(DiscImage),
}
