use super::{FormatError, FormatErrorKind as K, FormatResult, json};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const FORMAT_VERSION: u32 = 1;
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_INTEGRITY_BYTES: u64 = 64 * 1024;
pub const MAX_ARTWORK_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_IMAGE_SIDE: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;
pub const IMAGE_MEMORY_BUDGET: u64 = 256 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    pub disc_id: String,
    pub title: String,
    pub burned_at_unix: u64,
    pub tracks: Vec<TrackEntry>,
    pub appearance: Appearance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackEntry {
    pub path: String,
    pub container: String,
    pub codec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_track_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_track_total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_disc_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_disc_total: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub base_color: Rgb,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Artwork>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl Default for Rgb {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artwork {
    pub path: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    pub algorithm: String,
    pub entries: Vec<IntegrityEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrityEntry {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn parse_manifest(bytes: &[u8]) -> FormatResult<Manifest> {
    // Version preflight is intentionally independent of the future schema.
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(FormatError::new(
            K::ResourceLimit,
            "manifest exceeds byte limit",
        ));
    }
    #[derive(Deserialize)]
    struct Header {
        format_version: u64,
    }
    let header: Header = serde_json::from_slice(bytes)
        .map_err(|e| FormatError::new(K::InvalidJson, e.to_string()))?;
    if header.format_version != u64::from(FORMAT_VERSION) {
        return Err(FormatError::new(
            K::UnsupportedVersion,
            format!("unsupported VDISC version {}", header.format_version),
        ));
    }
    let manifest: Manifest = json::bind(json::parse(bytes, MAX_MANIFEST_BYTES)?)?;
    manifest.validate()?;
    Ok(manifest)
}
pub fn parse_integrity(bytes: &[u8]) -> FormatResult<Integrity> {
    let integrity: Integrity = json::bind(json::parse(bytes, MAX_INTEGRITY_BYTES)?)?;
    integrity.validate()?;
    Ok(integrity)
}

pub fn check_path(path: &str) -> FormatResult<()> {
    let valid = path == "manifest.json"
        || path == "integrity.json"
        || ["png", "jpeg", "webp"]
            .iter()
            .any(|e| path == format!("artwork/disc.{e}"))
        || (1..=6).any(|i| {
            ["flac", "mp3", "opus", "wav"]
                .iter()
                .any(|e| path == format!("tracks/{i:02}.{e}"))
        });
    if !valid {
        return Err(FormatError::new(K::UnsafePath, "not a canonical V1 archive path").at(path));
    }
    Ok(())
}
fn text(value: &str) -> FormatResult<()> {
    if value.trim().is_empty() {
        return Err(FormatError::new(K::InvalidSchema, "blank text"));
    }
    Ok(())
}
pub fn check_dimensions(width: u32, height: u32) -> FormatResult<()> {
    if width == 0 || height == 0 {
        return Err(FormatError::new(K::InvalidArtwork, "zero image dimension"));
    }
    if width > MAX_IMAGE_SIDE
        || height > MAX_IMAGE_SIDE
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err(FormatError::new(
            K::ResourceLimit,
            "image dimensions exceed V1 limits",
        ));
    }
    Ok(())
}

impl Manifest {
    /// Pure projection; the production burner must supply a fresh burn UUID.
    /// Preflight may use the draft UUID only for a discarded validation preview.
    pub fn from_draft(
        draft: &crate::DraftDisc,
        disc_id: Uuid,
        burned_at_unix: u64,
    ) -> FormatResult<Self> {
        let mut tracks = Vec::new();
        for (index, track) in draft.tracks().iter().enumerate() {
            let (container, extension) = match (track.container(), track.codec()) {
                ("flac", "flac") => ("flac", "flac"),
                ("mpa" | "mp3", "mp3") => ("mp3", "mp3"),
                ("ogg", "opus") => ("ogg", "opus"),
                ("wave" | "wav", "pcm") => ("wav", "wav"),
                _ => {
                    return Err(FormatError::new(
                        K::UnsupportedMedia,
                        "draft contains unsupported V1 audio",
                    ));
                }
            };
            tracks.push(TrackEntry {
                path: format!("tracks/{:02}.{extension}", index + 1),
                container: container.to_owned(),
                codec: track.codec().to_owned(),
                duration_ms: track.duration_ms(),
                sample_rate_hz: track.sample_rate(),
                channels: track.channels(),
                title: track.title().map(str::to_owned),
                artist: track.artist().map(str::to_owned),
                album: track.album().map(str::to_owned),
                genre: track.genre().map(str::to_owned),
                source_track_number: track.source_track_number(),
                source_track_total: track.source_track_total(),
                source_disc_number: track.source_disc_number(),
                source_disc_total: track.source_disc_total(),
            });
        }
        let appearance = draft.appearance();
        let color = appearance.base_color();
        let image = appearance.image().map(|image| Artwork {
            path: format!("artwork/disc.{}", image.format().as_str()),
            format: image.format().as_str().to_owned(),
            width: image.width(),
            height: image.height(),
        });
        let manifest = Self {
            format_version: FORMAT_VERSION,
            disc_id: disc_id.to_string(),
            title: draft.title().to_owned(),
            burned_at_unix,
            tracks,
            appearance: Appearance {
                base_color: Rgb {
                    r: color.r(),
                    g: color.g(),
                    b: color.b(),
                },
                label: appearance.label().map(str::to_owned),
                subtitle: appearance.subtitle().map(str::to_owned),
                image,
            },
        };
        manifest.validate()?;
        let encoded = serde_json::to_vec(&manifest)
            .map_err(|e| FormatError::new(K::InvalidSchema, e.to_string()))?;
        if encoded.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(FormatError::new(K::ResourceLimit, "manifest exceeds 1 MiB"));
        }
        Ok(manifest)
    }
    pub fn validate(&self) -> FormatResult<()> {
        if self.format_version != FORMAT_VERSION {
            return Err(FormatError::new(
                K::UnsupportedVersion,
                "unsupported format version",
            ));
        }
        let id = Uuid::parse_str(&self.disc_id)
            .map_err(|_| FormatError::new(K::InvalidSchema, "invalid disc UUID"))?;
        if id.get_version_num() != 4
            || id.to_string() != self.disc_id
            || id.get_variant() != uuid::Variant::RFC4122
        {
            return Err(FormatError::new(
                K::InvalidSchema,
                "disc ID must be canonical UUID v4",
            ));
        }
        text(&self.title)?;
        if !(1..=6).contains(&self.tracks.len()) {
            return Err(FormatError::new(
                K::InvalidSchema,
                "disc needs 1 to 6 tracks",
            ));
        }
        for (i, t) in self.tracks.iter().enumerate() {
            check_path(&t.path)?;
            let ext = match (t.container.as_str(), t.codec.as_str()) {
                ("flac", "flac") => "flac",
                ("mp3", "mp3") => "mp3",
                ("ogg", "opus") => "opus",
                ("wav", "pcm") => "wav",
                _ => {
                    return Err(FormatError::new(
                        K::UnsupportedMedia,
                        "unsupported container/codec pair",
                    )
                    .at(&t.path));
                }
            };
            if t.path != format!("tracks/{:02}.{ext}", i + 1) {
                return Err(FormatError::new(
                    K::InvalidSchema,
                    "track path disagrees with array position",
                )
                .at(&t.path));
            }
            if t.sample_rate_hz == Some(0) || t.channels == Some(0) {
                return Err(FormatError::new(K::InvalidSchema, "zero audio property").at(&t.path));
            }
            for v in [&t.title, &t.artist, &t.album, &t.genre]
                .into_iter()
                .flatten()
            {
                text(v).map_err(|e| e.at(&t.path))?;
            }
        }
        for v in [&self.appearance.label, &self.appearance.subtitle]
            .into_iter()
            .flatten()
        {
            text(v)?;
        }
        if let Some(a) = &self.appearance.image {
            check_path(&a.path)?;
            if !["png", "jpeg", "webp"].contains(&a.format.as_str())
                || a.path != format!("artwork/disc.{}", a.format)
            {
                return Err(
                    FormatError::new(K::InvalidArtwork, "artwork path/format mismatch").at(&a.path),
                );
            }
            check_dimensions(a.width, a.height).map_err(|e| e.at(&a.path))?;
        }
        Ok(())
    }
    pub(crate) fn covered_paths(&self) -> BTreeSet<&str> {
        let mut paths = BTreeSet::from(["manifest.json"]);
        paths.extend(self.tracks.iter().map(|t| t.path.as_str()));
        if let Some(a) = &self.appearance.image {
            paths.insert(&a.path);
        }
        paths
    }
}
impl Integrity {
    pub fn validate(&self) -> FormatResult<()> {
        if self.algorithm != "sha256" || !(2..=8).contains(&self.entries.len()) {
            return Err(FormatError::new(
                K::InvalidSchema,
                "invalid integrity algorithm/count",
            ));
        }
        let mut seen = BTreeSet::new();
        for e in &self.entries {
            check_path(&e.path)?;
            if e.path == "integrity.json" {
                return Err(FormatError::new(
                    K::InvalidSchema,
                    "integrity cannot cover itself",
                ));
            }
            if !seen.insert(&e.path) {
                return Err(
                    FormatError::new(K::DuplicateEntry, "duplicate integrity path").at(&e.path),
                );
            }
            if e.sha256.len() != 64
                || !e
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(
                    FormatError::new(K::InvalidSchema, "invalid lowercase SHA-256").at(&e.path),
                );
            }
        }
        Ok(())
    }
}
