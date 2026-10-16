//! VDISC V1 schema, validation, writer plumbing, and reader support.
//! Playback state and audio output remain separate concerns.
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
    sync::atomic::{AtomicBool, Ordering},
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

/// Stable payload location produced by the same bounded ZIP parser used by the
/// format validator. Consumers outside this crate never receive raw offsets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PayloadLocation {
    pub(crate) start: u64,
    pub(crate) len: u64,
}

/// Internal result used by the supported reader. Validation and ZIP indexing
/// happen once against the same already-open file handle.
#[derive(Debug)]
pub(crate) struct ValidatedArchive {
    pub(crate) validated: ValidatedFormat,
    pub(crate) payloads: BTreeMap<String, PayloadLocation>,
}

/// Validates a complete on-disk VDISC. Reads only; never extracts or mutates.
/// The result attests to this validation pass, not to future changes to the path.
pub fn validate_vdisc(path: impl AsRef<Path>) -> FormatResult<ValidatedFormat> {
    let file = File::open(path)?;
    Ok(validate_open_file(&file)?.validated)
}

/// Validate an already-open file descriptor and retain the payload locations
/// derived during that exact validation pass. This is the bridge used by the
/// Objective 15 reader so pathname replacement cannot switch the file later.
pub(crate) fn validate_open_file(file: &File) -> FormatResult<ValidatedArchive> {
    if !file.metadata()?.is_file() {
        return Err(FormatError::new(K::Io, "VDISC must be a regular file"));
    }

    // The clone refers to the same open file. Validation is allowed to seek;
    // reader payload access uses positioned I/O and does not depend on this
    // shared cursor afterwards.
    let mut reader = file.try_clone()?;
    validate_file(&mut reader, false, None)
}

/// Fast playback mount: index and check the complete manifest/integrity inventory,
/// hash and media-probe track one, and validate artwork. Other tracks remain
/// untrusted until the full validator completes on a background worker.
/// Does not replace or weaken validate_open_file().
pub(crate) fn validate_first_track_open_file(file: &File) -> FormatResult<ValidatedArchive> {
    if !file.metadata()?.is_file() {
        return Err(FormatError::new(K::Io, "VDISC must be a regular file"));
    }
    let mut reader = file.try_clone()?;
    validate_file(&mut reader, true, None)
}

/// Cancel-aware variant used only by the background playback worker. Strict
/// callers keep using validate_open_file without cancellation.
pub(crate) fn validate_open_file_with_cancel(
    file: &File,
    cancellation: &AtomicBool,
) -> FormatResult<ValidatedArchive> {
    if !file.metadata()?.is_file() {
        return Err(FormatError::new(K::Io, "VDISC must be a regular file"));
    }
    let mut reader = file.try_clone()?;
    validate_file(&mut reader, false, Some(cancellation))
}

fn check_cancellation(cancellation: Option<&AtomicBool>) -> FormatResult<()> {
    if cancellation.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err(FormatError::new(
            K::ResourceLimit,
            "background verification cancelled after disc removal",
        ));
    }
    Ok(())
}

fn validate_file(
    file: &mut File,
    first_track_only: bool,
    cancellation: Option<&AtomicBool>,
) -> FormatResult<ValidatedArchive> {
    check_cancellation(cancellation)?;
    let entries = zip::index(file)?;
    let index: BTreeMap<&str, &Entry> = entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let get = |name: &str| {
        index.get(name).copied().ok_or_else(|| {
            FormatError::new(K::MissingEntry, "required archive entry missing").at(name)
        })
    };
    let manifest = parse_manifest(&read_small(
        file,
        get("manifest.json")?,
        MAX_MANIFEST_BYTES,
    )?)
    .map_err(|e| e.at("manifest.json"))?;
    let integrity = parse_integrity(&read_small(
        file,
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
        check_cancellation(cancellation)?;
        let entry = get(name)?;
        let record = records[name];
        if record.size_bytes != entry.size {
            return Err(FormatError::new(
                K::IntegrityMismatch,
                "declared size differs from ZIP size",
            )
            .at(name));
        }
        // Fast mount hashes every non-audio entry and only the first track.
        // Crucially, CRC, size, schema and local-header checks still apply to
        // the *entire* archive index. No other track bytes are trusted here.
        let required_now = !first_track_only
            || *name == "manifest.json"
            || manifest
                .tracks
                .first()
                .is_some_and(|track| track.path == *name)
            || manifest
                .appearance
                .image
                .as_ref()
                .is_some_and(|image| image.path == *name);
        if required_now {
            let hash = hash_entry(file, entry, cancellation)?;
            if hash != record.sha256 {
                return Err(FormatError::new(K::IntegrityMismatch, "SHA-256 mismatch").at(name));
            }
        }
    }
    check_cancellation(cancellation)?;
    if let Some(image) = &manifest.appearance.image {
        let bytes = read_small(file, get(&image.path)?, MAX_ARTWORK_BYTES)?;
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
    for track in manifest
        .tracks
        .iter()
        .take(if first_track_only { 1 } else { usize::MAX })
    {
        check_cancellation(cancellation)?;
        media::validate_audio(file, get(&track.path)?, track)?;
    }

    check_cancellation(cancellation)?;
    let payloads = entries
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                PayloadLocation {
                    start: entry.data,
                    len: entry.size,
                },
            )
        })
        .collect();

    Ok(ValidatedArchive {
        validated: ValidatedFormat {
            manifest,
            integrity,
        },
        payloads,
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

fn hash_entry(
    file: &mut File,
    entry: &Entry,
    cancellation: Option<&AtomicBool>,
) -> FormatResult<String> {
    check_cancellation(cancellation)?;
    file.seek(SeekFrom::Start(entry.data))?;
    let mut remaining = entry.size;
    let mut buffer = [0; 64 * 1024];
    let mut sha = Sha256::new();
    let mut crc = u32::MAX;
    while remaining != 0 {
        check_cancellation(cancellation)?;
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
