use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::format::{
    self, Appearance, FormatError, FormatErrorKind, FormatResult, Integrity, Manifest,
    PayloadLocation, TrackEntry, ValidatedFormat,
};

/// The supported read-only consumer view of a finalized VDISC.
///
/// Opening performs full VDISC validation before this value is returned. The
/// file descriptor remains open for the lifetime of the value, so replacing
/// the pathname later does not switch payload access to another file.
///
/// External writes to the already-open inode cannot be prevented by this API.
/// Call `verify_open_file()` to revalidate that same inode. Payload views that
/// were already handed out remain bounded to their original entry range, but a
/// concurrent external writer may still change bytes inside that range.
#[derive(Debug, Clone)]
pub struct BurnedDisc {
    path: PathBuf,
    file: Arc<File>,
    validated: ValidatedFormat,
    payloads: BTreeMap<String, PayloadLocation>,
}

impl BurnedDisc {
    /// Open one finished disc using the full VDISC validator.
    pub fn open(path: impl AsRef<Path>) -> FormatResult<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let archive = format::validate_open_file(&file)?;

        Ok(Self {
            path,
            file: Arc::new(file),
            validated: archive.validated,
            payloads: archive.payloads,
        })
    }

    /// Backward-compatible Objective 14 spelling for opening a burned disc.
    pub fn inspect(path: impl AsRef<Path>) -> FormatResult<Self> {
        Self::open(path)
    }

    /// The path used when this disc was opened. It is informational only; all
    /// payload access uses the retained file descriptor, not a path reopen.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disc_id(&self) -> &str {
        &self.validated.manifest().disc_id
    }

    pub fn title(&self) -> &str {
        &self.validated.manifest().title
    }

    pub fn burned_at_unix(&self) -> u64 {
        self.validated.manifest().burned_at_unix
    }

    pub fn track_count(&self) -> usize {
        self.validated.manifest().tracks.len()
    }

    pub fn tracks(&self) -> &[TrackEntry] {
        &self.validated.manifest().tracks
    }

    pub fn appearance(&self) -> &Appearance {
        &self.validated.manifest().appearance
    }

    pub fn manifest(&self) -> &Manifest {
        self.validated.manifest()
    }

    pub fn integrity(&self) -> &Integrity {
        self.validated.integrity()
    }

    /// Open a bounded, seekable view over the zero-based track index.
    ///
    /// The returned view can never seek or read outside this track's validated
    /// ZIP entry. `Ok(None)` means the requested track index does not exist.
    pub fn track_payload(&self, index: usize) -> FormatResult<Option<DiscPayload>> {
        let Some(track) = self.tracks().get(index) else {
            return Ok(None);
        };

        self.payload(&track.path).map(Some)
    }

    /// Return the exact embedded artwork bytes, if this disc has artwork.
    /// Artwork remains bounded by the VDISC V1 20 MiB validation limit.
    pub fn artwork_bytes(&self) -> FormatResult<Option<Vec<u8>>> {
        let Some(image) = &self.appearance().image else {
            return Ok(None);
        };

        let location = self.location(&image.path)?;
        if location.len > format::MAX_ARTWORK_BYTES {
            return Err(FormatError::new(
                FormatErrorKind::ResourceLimit,
                "artwork exceeds V1 limit",
            )
            .at(&image.path));
        }

        let size = usize::try_from(location.len).map_err(|_| {
            FormatError::new(FormatErrorKind::ResourceLimit, "artwork is too large")
        })?;
        let mut bytes = vec![0; size];
        let mut payload = DiscPayload::new(Arc::clone(&self.file), location);
        payload.read_exact(&mut bytes)?;

        Ok(Some(bytes))
    }

    /// Revalidate the exact file descriptor retained by this reader.
    ///
    /// This detects in-place external modification, including a rewrite that
    /// produces a different otherwise-valid VDISC.
    pub fn verify_open_file(&self) -> FormatResult<()> {
        let current = format::validate_open_file(self.file.as_ref())?;
        self.verify_same_snapshot(&current.validated)
    }

    /// Reopen the original path and verify it still contains this exact disc.
    ///
    /// Unlike payload access, this intentionally follows the pathname again so
    /// callers can detect rename/replacement of the original path.
    pub fn verify_current_path(&self) -> FormatResult<()> {
        let current = format::validate_vdisc(&self.path)?;
        self.verify_same_snapshot(&current)
    }

    fn payload(&self, path: &str) -> FormatResult<DiscPayload> {
        let location = self.location(path)?;
        Ok(DiscPayload::new(Arc::clone(&self.file), location))
    }

    fn location(&self, path: &str) -> FormatResult<PayloadLocation> {
        self.payloads.get(path).copied().ok_or_else(|| {
            FormatError::new(
                FormatErrorKind::MissingEntry,
                "validated payload location is missing",
            )
            .at(path)
        })
    }

    fn verify_same_snapshot(&self, current: &ValidatedFormat) -> FormatResult<()> {
        if current.manifest() != self.manifest() || current.integrity() != self.integrity() {
            return Err(FormatError::new(
                FormatErrorKind::IntegrityMismatch,
                "VDISC no longer matches the opened validated snapshot",
            ));
        }

        Ok(())
    }
}

/// Bounded seekable access to one embedded VDISC payload.
///
/// The payload holds the same open inode as its parent `BurnedDisc`; it does
/// not reopen the filesystem path. Positioned reads keep independent payload
/// views from sharing or racing on a global file cursor.
#[derive(Debug)]
pub struct DiscPayload {
    file: Arc<File>,
    start: u64,
    len: u64,
    pos: u64,
}

impl DiscPayload {
    fn new(file: Arc<File>, location: PayloadLocation) -> Self {
        Self {
            file,
            start: location.start,
            len: location.len,
            pos: 0,
        }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn position(&self) -> u64 {
        self.pos
    }
}

impl Read for DiscPayload {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.pos == self.len {
            return Ok(0);
        }

        let count = (self.len - self.pos).min(buffer.len() as u64) as usize;
        let offset = self
            .start
            .checked_add(self.pos)
            .ok_or_else(|| io::Error::other("embedded payload offset overflow"))?;
        let read = self.file.as_ref().read_at(&mut buffer[..count], offset)?;

        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "embedded payload was truncated after the disc was opened",
            ));
        }

        self.pos += read as u64;
        Ok(read)
    }
}

impl Seek for DiscPayload {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(delta) => i128::from(self.pos) + i128::from(delta),
            SeekFrom::End(delta) => i128::from(self.len) + i128::from(delta),
        };

        if position < 0 || position > i128::from(self.len) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside embedded VDISC payload",
            ));
        }

        self.pos = position as u64;
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::DiscPayload;
    use crate::format::PayloadLocation;
    use std::{io::Read, os::unix::fs::FileExt, sync::Arc};

    #[test]
    fn payload_supports_offsets_beyond_four_gib_without_large_allocation() {
        let file = tempfile::tempfile().unwrap();
        let start = u64::from(u32::MAX) + 8192;
        file.set_len(start + 3).unwrap();
        file.write_at(b"zip", start).unwrap();

        let mut payload = DiscPayload::new(Arc::new(file), PayloadLocation { start, len: 3 });
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes).unwrap();

        assert_eq!(bytes, b"zip");
        assert_eq!(payload.position(), 3);
    }
}
