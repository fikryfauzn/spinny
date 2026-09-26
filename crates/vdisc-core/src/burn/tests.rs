use super::{
    linux::{self, Hooks, Point},
    *,
};
use crate::{
    AddTrackRequest, DraftDisc, TrackSourceKind, ValidatedLocalAudio, add_validated_local_track,
    format::validate_vdisc, save_draft,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};
#[path = "../../tests/support/vdisc_fixture.rs"]
mod fixture;
fn ready(dir: &Path) -> (PathBuf, PathBuf) {
    let source = dir.join("song.wav");
    fs::write(&source, fixture::wav()).unwrap();
    let draft = dir.join("disc.vdraft");
    save_draft(&draft, &DraftDisc::new("Disc").unwrap()).unwrap();
    let selected = AddTrackRequest::begin(&draft)
        .unwrap()
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source)
        .unwrap();
    add_validated_local_track(ValidatedLocalAudio::validate(selected).unwrap()).unwrap();
    (draft, source)
}
fn no_temps(dir: &Path) {
    assert!(fs::read_dir(dir).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".vdisc-burn-")
    }));
}
struct FailAt {
    point: Point,
    cleanup: bool,
}
impl Hooks for FailAt {
    fn at(&mut self, p: Point, _: &Path) -> io::Result<()> {
        if p == self.point || (self.cleanup && p == Point::Cleanup) {
            Err(io::Error::other(format!("injected {p:?}")))
        } else {
            Ok(())
        }
    }
}
#[test]
fn phase_failures_never_publish_and_clean_up() {
    for (point, phase) in [
        (Point::Clock, BurnPhase::Clock),
        (Point::AfterPreflight, BurnPhase::Preflight),
        (Point::BeforeWrite, BurnPhase::Write),
        (Point::FileSync, BurnPhase::FileSync),
        (Point::Verify, BurnPhase::Verify),
        (Point::Publish, BurnPhase::Publish),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (draft, source) = ready(dir.path());
        let out = dir.path().join("out.vdisc");
        let original = fs::read(&draft).unwrap();
        let error = linux::burn_with(
            &draft,
            &out,
            &mut FailAt {
                point,
                cleanup: false,
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, phase);
        assert!(!out.exists());
        no_temps(dir.path());
        assert_eq!(fs::read(draft).unwrap(), original);
        assert_eq!(fs::read(source).unwrap(), fixture::wav());
    }
}
#[test]
fn directory_sync_failure_reports_published_valid_disc() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let out = dir.path().join("out.vdisc");
    let result = linux::burn_with(
        &draft,
        &out,
        &mut FailAt {
            point: Point::DirectorySync,
            cleanup: false,
        },
    )
    .unwrap();
    assert!(matches!(
        result.durability,
        BurnDurability::PublishedButDirectorySyncFailed { .. }
    ));
    validate_vdisc(out).unwrap();
    no_temps(dir.path());
}
#[test]
fn cleanup_failure_preserves_original_error_and_leftover_path() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let out = dir.path().join("out.vdisc");
    let error = linux::burn_with(
        &draft,
        &out,
        &mut FailAt {
            point: Point::Verify,
            cleanup: true,
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, BurnPhase::Verify);
    assert!(error.diagnostic.contains("Verify"));
    let cleanup = error.cleanup.unwrap();
    assert!(cleanup.diagnostic.contains("Cleanup"));
    assert!(cleanup.temporary_path.exists());
    assert!(!out.exists());
    fs::remove_file(cleanup.temporary_path).unwrap();
}
struct Mutate {
    point: Point,
    mode: u8,
}
impl Hooks for Mutate {
    fn at(&mut self, p: Point, path: &Path) -> io::Result<()> {
        if p == self.point {
            let mut bytes = fs::read(path)?;
            match self.mode {
                0 => {
                    let n = bytes.len();
                    bytes[n - 1] ^= 1;
                }
                1 => {
                    bytes.pop();
                }
                2 => bytes.push(1),
                _ => bytes = b"invalid draft".to_vec(),
            }
            fs::write(path, bytes)?;
        }
        Ok(())
    }
}
#[test]
fn copied_source_changes_are_rejected_even_after_successful_preflight() {
    for mode in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let (draft, _) = ready(dir.path());
        let out = dir.path().join("out.vdisc");
        let error = linux::burn_with(
            &draft,
            &out,
            &mut Mutate {
                point: Point::BeforeCopy,
                mode,
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, BurnPhase::Source);
        assert!(!out.exists());
        no_temps(dir.path());
    }
}
#[test]
fn temporary_corruption_is_rejected_before_publication() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let out = dir.path().join("out.vdisc");
    let error = linux::burn_with(
        &draft,
        &out,
        &mut Mutate {
            point: Point::Verify,
            mode: 0,
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, BurnPhase::Verify);
    assert!(!out.exists());
    no_temps(dir.path());
}
#[test]
fn draft_replacement_after_snapshot_does_not_reload_draft() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let snapshot = crate::load_draft(&draft).unwrap();
    fs::write(&draft, b"invalid draft").unwrap();
    let report =
        crate::preflight::preflight_snapshot(&snapshot, &draft, &dir.path().join("out.vdisc"));
    assert!(report.is_ready(), "{:?}", report.issues());
    assert!(!crate::run_preflight(&draft, dir.path().join("out.vdisc")).is_ready());
    save_draft(&draft, &snapshot).unwrap();
    let result = linux::burn_with(
        &draft,
        &dir.path().join("out.vdisc"),
        &mut Mutate {
            point: Point::AfterPreflight,
            mode: 3,
        },
    )
    .unwrap();
    assert_eq!(
        validate_vdisc(result.output_path).unwrap().manifest().title,
        snapshot.title()
    );
    assert_eq!(fs::read(draft).unwrap(), b"invalid draft");
}
struct Race(Arc<Barrier>);
impl Hooks for Race {
    fn at(&mut self, p: Point, _: &Path) -> io::Result<()> {
        if p == Point::Publish {
            self.0.wait();
        }
        Ok(())
    }
}
#[test]
fn concurrent_publication_has_exactly_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let out = dir.path().join("out.vdisc");
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let draft = draft.clone();
            let out = out.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || linux::burn_with(&draft, &out, &mut Race(barrier)))
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().find_map(|r| r.as_ref().err()).unwrap().phase,
        BurnPhase::Publish
    );
    let winning_id = results
        .iter()
        .find_map(|r| r.as_ref().ok())
        .unwrap()
        .disc_id
        .to_string();
    assert_eq!(validate_vdisc(out).unwrap().manifest().disc_id, winning_id);
    no_temps(dir.path());
}
#[test]
fn invalid_draft_extension_remains_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let other = dir.path().join("disc.json");
    fs::rename(draft, &other).unwrap();
    assert!(!crate::run_preflight(&other, dir.path().join("out.vdisc")).is_ready());
    assert!(burn(other, dir.path().join("out.vdisc")).is_err());
}

#[test]
fn output_payload_failure_reports_write_phase_and_destination() {
    use std::io::{Cursor, Write};
    struct Disk {
        left: usize,
    }
    impl Write for Disk {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.left == 0 {
                return Err(io::Error::from_raw_os_error(libc::ENOSPC));
            }
            let n = self.left.min(b.len());
            self.left -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    // Header is 63 bytes; fail after seven payload bytes, not before copying.
    let mut writer = crate::format::writer::StoredZipWriter::new(Disk { left: 70 });
    let bytes = [1; 100];
    let expected = crate::SourceFingerprint::from_bytes(&bytes);
    let error = linux::copy_checked(
        &mut writer,
        &mut Cursor::new(bytes),
        Path::new("source.wav"),
        "tracks/01.wav",
        &expected,
        Path::new("out.vdisc"),
    )
    .unwrap_err();
    assert_eq!(error.phase, BurnPhase::Write);
    assert_eq!(error.path, Path::new("out.vdisc"));
    assert!(writer.finish().is_err());
}
#[test]
fn input_io_failure_still_reports_source_phase_and_path() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("source read failed"))
        }
    }
    let mut writer = crate::format::writer::StoredZipWriter::new(Vec::new());
    let expected = crate::SourceFingerprint::from_bytes(&[1]);
    let error = linux::copy_checked(
        &mut writer,
        &mut Broken,
        Path::new("source.wav"),
        "tracks/01.wav",
        &expected,
        Path::new("out.vdisc"),
    )
    .unwrap_err();
    assert_eq!(error.phase, BurnPhase::Source);
    assert_eq!(error.path, Path::new("source.wav"));
}
