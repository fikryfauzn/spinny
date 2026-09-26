//! VDISC V1 schema and validation contract. No production burner or playback API.
mod artwork;
mod error;
mod json;
pub(crate) mod media;
mod schema;
pub(crate) mod writer;
mod zip;

pub use artwork::{ImageInfo, image_worker_main, inspect_artwork};
pub use error::{FormatError, FormatErrorKind, FormatResult};
#[doc(hidden)]
pub use media::media_worker_main;
pub use schema::*;

use FormatErrorKind as K;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use zip::Entry;

#[derive(Debug, Clone)]
pub struct ValidatedFormat {
    manifest: Manifest,
    integrity: Integrity,
}
impl ValidatedFormat {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn integrity(&self) -> &Integrity {
        &self.integrity
    }
}

/// Validates a complete on-disk VDISC. Reads only; never extracts or mutates.
/// The result attests to this validation pass, not to future changes to the path.
pub fn validate_vdisc(path: impl AsRef<Path>) -> FormatResult<ValidatedFormat> {
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(FormatError::new(K::Io, "VDISC must be a regular file"));
    }
    let entries = zip::index(&mut file)?;
    let index: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let get = |name: &str| {
        index.get(name).copied().ok_or_else(|| {
            FormatError::new(K::MissingEntry, "required archive entry missing").at(name)
        })
    };
    let manifest = parse_manifest(&read_small(
        &mut file,
        get("manifest.json")?,
        MAX_MANIFEST_BYTES,
    )?)
    .map_err(|e| e.at("manifest.json"))?;
    let integrity = parse_integrity(&read_small(
        &mut file,
        get("integrity.json")?,
        MAX_INTEGRITY_BYTES,
    )?)
    .map_err(|e| e.at("integrity.json"))?;
    let covered = manifest.covered_paths();
    for name in &covered {
        get(name)?;
    }
    for name in index.keys() {
        if *name != "integrity.json" && !covered.contains(name) {
            return Err(FormatError::new(
                K::UnexpectedEntry,
                "archive contains unreferenced entry",
            )
            .at(name));
        }
    }
    let records: BTreeMap<&str, &IntegrityEntry> = integrity
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();
    for name in &covered {
        if !records.contains_key(name) {
            return Err(FormatError::new(K::MissingEntry, "integrity record missing").at(name));
        }
    }
    for name in records.keys() {
        if !covered.contains(name) {
            return Err(
                FormatError::new(K::UnexpectedEntry, "unreferenced integrity record").at(name),
            );
        }
    }
    if let Some(image) = &manifest.appearance.image
        && get(&image.path)?.size > MAX_ARTWORK_BYTES
    {
        return Err(
            FormatError::new(K::ResourceLimit, "encoded artwork exceeds 20 MiB").at(&image.path),
        );
    }
    for name in &covered {
        let entry = get(name)?;
        let record = records[name];
        if record.size_bytes != entry.size {
            return Err(FormatError::new(
                K::IntegrityMismatch,
                "declared size differs from ZIP size",
            )
            .at(name));
        }
        let hash = hash_entry(&mut file, entry)?;
        if hash != record.sha256 {
            return Err(FormatError::new(K::IntegrityMismatch, "SHA-256 mismatch").at(name));
        }
    }
    if let Some(image) = &manifest.appearance.image {
        let bytes = read_small(&mut file, get(&image.path)?, MAX_ARTWORK_BYTES)?;
        let actual = inspect_artwork(&bytes).map_err(|e| e.at(&image.path))?;
        if actual.format != image.format
            || actual.width != image.width
            || actual.height != image.height
        {
            return Err(FormatError::new(
                K::InvalidArtwork,
                "image content disagrees with manifest",
            )
            .at(&image.path));
        }
    }
    for track in &manifest.tracks {
        media::validate_audio(&file, get(&track.path)?, track)?;
    }
    Ok(ValidatedFormat {
        manifest,
        integrity,
    })
}

fn read_small(file: &mut File, entry: &Entry, limit: u64) -> FormatResult<Vec<u8>> {
    if entry.size > limit {
        return Err(FormatError::new(K::ResourceLimit, "entry exceeds byte limit").at(&entry.name));
    }
    let size = usize::try_from(entry.size)
        .map_err(|_| FormatError::new(K::ResourceLimit, "entry cannot fit in memory"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| FormatError::new(K::ResourceLimit, "allocation failed"))?;
    bytes.resize(size, 0);
    file.seek(SeekFrom::Start(entry.data))?;
    file.read_exact(&mut bytes)?;
    if !zip::crc_update(u32::MAX, &bytes) != entry.crc {
        return Err(FormatError::new(K::IntegrityMismatch, "ZIP CRC32 mismatch").at(&entry.name));
    }
    Ok(bytes)
}
fn hash_entry(file: &mut File, entry: &Entry) -> FormatResult<String> {
    file.seek(SeekFrom::Start(entry.data))?;
    let mut remaining = entry.size;
    let mut buffer = [0; 64 * 1024];
    let mut sha = Sha256::new();
    let mut crc = u32::MAX;
    while remaining != 0 {
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        sha.update(&buffer[..count]);
        crc = zip::crc_update(crc, &buffer[..count]);
        remaining -= count as u64;
    }
    if !crc != entry.crc {
        return Err(FormatError::new(K::IntegrityMismatch, "ZIP CRC32 mismatch").at(&entry.name));
    }
    let digest = sha.finalize();
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}
