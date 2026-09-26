//! Synchronous, no-replace VDISC publication. Inspect the returned durability.
mod error;
#[cfg(target_os = "linux")]
mod publication;
pub use error::{BurnError, BurnPhase, CleanupFailure};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BurnDurability {
    Synced,
    /// The disc exists. Do not retry as if nothing was published.
    PublishedButDirectorySyncFailed {
        diagnostic: String,
    },
}
#[derive(Debug)]
#[must_use = "inspect burn durability before reporting durable success"]
pub struct BurnResult {
    pub disc_id: Uuid,
    pub burned_at_unix: u64,
    /// Resolved name at start; not a permanent locator if its parent is renamed.
    pub output_path: PathBuf,
    pub track_count: usize,
    pub size_bytes: u64,
    pub durability: BurnDurability,
}
/// Load one draft snapshot and synchronously publish a verified disc.
/// Existing destinations are never replaced. An error never means that a competing
/// process did not publish there. A returned result always means we published.
pub fn burn(
    draft_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<BurnResult, BurnError> {
    #[cfg(target_os = "linux")]
    {
        linux::burn_with(
            draft_path.as_ref(),
            output_path.as_ref(),
            &mut linux::NoHooks,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = draft_path;
        Err(BurnError::new(
            BurnPhase::Publish,
            output_path.as_ref(),
            "bounded burner currently requires Linux",
        ))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::publication::Publication;
    use super::*;
    use crate::{
        DraftDisc, SourceFingerprint,
        format::{self, Integrity, IntegrityEntry, Manifest, writer::StoredZipWriter},
        load_draft,
        preflight::preflight_snapshot,
    };
    use std::{
        fs::{File, OpenOptions},
        io::{self, Cursor, Read, Write},
        os::unix::fs::OpenOptionsExt,
        time::{SystemTime, UNIX_EPOCH},
    };

    // Internal deterministic fault seams; no public flags or global test state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum Point {
        Clock,
        AfterPreflight,
        BeforeWrite,
        BeforeCopy,
        FileSync,
        Verify,
        Publish,
        DirectorySync,
        Cleanup,
    }
    pub(super) trait Hooks {
        fn at(&mut self, point: Point, path: &Path) -> io::Result<()>;
    }
    pub(super) struct NoHooks;
    impl Hooks for NoHooks {
        fn at(&mut self, _: Point, _: &Path) -> io::Result<()> {
            Ok(())
        }
    }
    pub(super) fn burn_with(
        draft_path: &Path,
        output_path: &Path,
        hooks: &mut impl Hooks,
    ) -> Result<BurnResult, BurnError> {
        let cwd =
            std::env::current_dir().map_err(|e| BurnError::new(BurnPhase::Load, draft_path, e))?;
        let draft_path = cwd.join(draft_path);
        let output_path = cwd.join(output_path);
        let draft =
            load_draft(&draft_path).map_err(|e| BurnError::new(BurnPhase::Load, &draft_path, e))?;
        let mut publication = match Publication::prepare(&output_path) {
            Ok(publication) => publication,
            Err(cause) => {
                let mut error = BurnError::new(BurnPhase::Preflight, &output_path, cause);
                error.preflight = Some(Box::new(preflight_snapshot(
                    &draft,
                    &draft_path,
                    &output_path,
                )));
                return Err(error);
            }
        };
        // Validate against the pinned directory, including an existing dangling symlink.
        let report = preflight_snapshot(&draft, &draft_path, &publication.destination_path());
        if !report.is_ready() {
            let mut error = BurnError::new(
                BurnPhase::Preflight,
                &output_path,
                "burn preflight rejected the draft or destination",
            );
            error.preflight = Some(Box::new(report));
            return Err(error);
        }
        hooks
            .at(Point::AfterPreflight, &draft_path)
            .map_err(|e| BurnError::new(BurnPhase::Preflight, &draft_path, e))?;
        hooks
            .at(Point::Clock, &output_path)
            .map_err(|e| BurnError::new(BurnPhase::Clock, &output_path, e))?;
        let burned_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| BurnError::new(BurnPhase::Clock, &output_path, e))?
            .as_secs();
        let disc_id = Uuid::new_v4();
        let manifest = Manifest::from_draft(&draft, disc_id, burned_at_unix)
            .map_err(|e| BurnError::new(BurnPhase::Preflight, &output_path, e))?;
        publication
            .create_temp()
            .map_err(|e| BurnError::new(BurnPhase::Write, &output_path, e))?;
        let outcome = build_and_publish(&draft, &manifest, disc_id, &mut publication, hooks);
        match outcome {
            Ok(result) => Ok(result),
            Err(mut error) => {
                let path = publication.temporary_path();
                let cleanup = hooks
                    .at(Point::Cleanup, &path)
                    .and_then(|()| publication.cleanup());
                if let Err(cause) = cleanup {
                    publication.leave_temporary();
                    error.cleanup = Some(Box::new(CleanupFailure {
                        temporary_path: path,
                        diagnostic: cause.to_string(),
                    }));
                }
                Err(error)
            }
        }
    }
    fn build_and_publish(
        draft: &DraftDisc,
        manifest: &Manifest,
        disc_id: Uuid,
        publication: &mut Publication,
        hooks: &mut impl Hooks,
    ) -> Result<BurnResult, BurnError> {
        let output = publication.output_path.clone();
        let temp = publication
            .validation_path()
            .map_err(|e| BurnError::new(BurnPhase::Write, &output, e))?;
        hooks
            .at(Point::BeforeWrite, &temp)
            .map_err(|e| BurnError::new(BurnPhase::Write, &output, e))?;
        let size_bytes = write_archive(
            draft,
            manifest,
            publication
                .temp_file_mut()
                .map_err(|e| BurnError::new(BurnPhase::Write, &output, e))?,
            hooks,
            &output,
        )?;
        hooks
            .at(Point::FileSync, &temp)
            .and_then(|()| publication.temp_file_mut()?.sync_all())
            .map_err(|e| BurnError::new(BurnPhase::FileSync, &output, e))?;
        hooks
            .at(Point::Verify, &temp)
            .map_err(|e| BurnError::new(BurnPhase::Verify, &output, e))?;
        let checked = format::validate_vdisc(&temp)
            .map_err(|e| BurnError::new(BurnPhase::Verify, &output, e))?;
        if checked.manifest() != manifest {
            return Err(BurnError::new(
                BurnPhase::Verify,
                &output,
                "verified manifest differs from snapshot",
            ));
        }
        // Result allocations precede publication. Post-publication I/O failure is an outcome.
        let mut result = BurnResult {
            disc_id,
            burned_at_unix: manifest.burned_at_unix,
            output_path: output,
            track_count: manifest.tracks.len(),
            size_bytes,
            durability: BurnDurability::Synced,
        };
        hooks
            .at(Point::Publish, &temp)
            .and_then(|()| publication.publish())
            .map_err(|e| BurnError::new(BurnPhase::Publish, &result.output_path, e))?;
        if let Err(error) = hooks
            .at(Point::DirectorySync, &result.output_path)
            .and_then(|()| publication.sync_directory())
        {
            result.durability = BurnDurability::PublishedButDirectorySyncFailed {
                diagnostic: error.to_string(),
            };
        }
        Ok(result)
    }
    fn write_archive(
        draft: &DraftDisc,
        manifest: &Manifest,
        file: &mut File,
        hooks: &mut impl Hooks,
        output: &Path,
    ) -> Result<u64, BurnError> {
        let failure = |e| BurnError::new(BurnPhase::Write, output, e);
        let mut writer = StoredZipWriter::new(file);
        let bytes = serde_json::to_vec(manifest).map_err(|e| failure(e.to_string()))?;
        if bytes.len() as u64 > format::MAX_MANIFEST_BYTES {
            return Err(failure("manifest exceeds limit".into()));
        }
        let mut records = vec![
            writer
                .write_entry(
                    "manifest.json",
                    &mut Cursor::new(&bytes),
                    bytes.len() as u64,
                )
                .map_err(|e| failure(e.to_string()))?
                .record,
        ];
        for (track, projected) in draft.tracks().iter().zip(&manifest.tracks) {
            let expected = track.source_fingerprint().ok_or_else(|| {
                BurnError::new(
                    BurnPhase::Source,
                    track.source_path(),
                    "missing fingerprint",
                )
            })?;
            records.push(copy_source(
                &mut writer,
                track.source_path(),
                &projected.path,
                expected,
                hooks,
                output,
            )?);
        }
        if let (Some(image), Some(projected)) =
            (draft.appearance().image(), &manifest.appearance.image)
        {
            records.push(copy_source(
                &mut writer,
                image.source_path(),
                &projected.path,
                image.source_fingerprint(),
                hooks,
                output,
            )?);
        }
        let integrity = Integrity {
            algorithm: "sha256".into(),
            entries: records,
        };
        integrity.validate().map_err(|e| failure(e.to_string()))?;
        let bytes = serde_json::to_vec(&integrity).map_err(|e| failure(e.to_string()))?;
        if bytes.len() as u64 > format::MAX_INTEGRITY_BYTES {
            return Err(failure("integrity exceeds limit".into()));
        }
        writer
            .write_entry(
                "integrity.json",
                &mut Cursor::new(&bytes),
                bytes.len() as u64,
            )
            .map_err(|e| failure(e.to_string()))?;
        let (_, size) = writer.finish().map_err(|e| failure(e.to_string()))?;
        Ok(size)
    }
    fn copy_source(
        writer: &mut StoredZipWriter<&mut File>,
        source: &Path,
        name: &str,
        expected: &SourceFingerprint,
        hooks: &mut impl Hooks,
        output: &Path,
    ) -> Result<IntegrityEntry, BurnError> {
        let fail = |e: String| BurnError::new(BurnPhase::Source, source, e);
        hooks
            .at(Point::BeforeCopy, source)
            .map_err(|e| fail(e.to_string()))?;
        // NONBLOCK avoids hanging if a source was swapped for a FIFO after preflight.
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(source)
            .map_err(|e| fail(e.to_string()))?;
        let metadata = file.metadata().map_err(|e| fail(e.to_string()))?;
        if !metadata.is_file() || metadata.len() != expected.size_bytes() {
            return Err(fail("source type or length changed".into()));
        }
        copy_checked(writer, &mut file, source, name, expected, output)
    }
    struct TrackedRead<R> {
        inner: R,
        failed: bool,
    }
    impl<R: Read> Read for TrackedRead<R> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let result = self.inner.read(bytes);
            if result
                .as_ref()
                .is_err_and(|error| error.kind() != io::ErrorKind::Interrupted)
            {
                self.failed = true;
            }
            result
        }
    }
    pub(super) fn copy_checked<W: Write>(
        writer: &mut StoredZipWriter<W>,
        input: &mut impl Read,
        source: &Path,
        name: &str,
        expected: &SourceFingerprint,
        output: &Path,
    ) -> Result<IntegrityEntry, BurnError> {
        let mut reader = TrackedRead {
            inner: input,
            failed: false,
        };
        let receipt = writer
            .write_entry(name, &mut reader, expected.size_bytes())
            .map_err(|error| {
                if reader.failed || error.kind == format::FormatErrorKind::IntegrityMismatch {
                    BurnError::new(BurnPhase::Source, source, error)
                } else {
                    BurnError::new(BurnPhase::Write, output, error)
                }
            })?;
        let record = receipt.record;
        if record.size_bytes != expected.size_bytes() || record.sha256 != expected.sha256() {
            return Err(BurnError::new(
                BurnPhase::Source,
                source,
                "copied bytes do not match imported fingerprint",
            ));
        }
        Ok(record)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
