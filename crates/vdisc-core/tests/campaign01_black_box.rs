#![cfg(target_os = "linux")]

use std::{
    env,
    error::Error,
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use vdisc_core::{
    AddTrackRequest, BurnedDisc, CustomizationSession, DiscColor, DraftDisc, LinuxAudioPlayer,
    PlaybackBackend, PlaybackBackendEvent, PlaybackPlayer, PlaybackResult, PlaybackSession,
    PlayerState, PreflightIssueCode, TrackSourceKind, ValidatedLocalAudio, VdiscError,
    add_validated_local_track, burn, format::validate_vdisc, load_draft, move_draft_track,
    remove_draft_track, run_preflight, save_draft,
};

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;

const RESTART_CHILD: &str = "VDISC_CAMPAIGN01_RESTART_CHILD";
const RESTART_DISC: &str = "VDISC_CAMPAIGN01_RESTART_DISC";
const HARDWARE_CHILD: &str = "VDISC_CAMPAIGN01_HARDWARE_CHILD";
const HARDWARE_DISC: &str = "VDISC_CAMPAIGN01_HARDWARE_DISC";

fn audio_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/audio")
        .join(name)
}

fn vdisc_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc")
        .join(name)
}

fn import_track(draft_path: &Path, source_path: &Path) -> TestResult {
    let selected = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(source_path)?;
    let validated = ValidatedLocalAudio::validate(selected)?;
    add_validated_local_track(validated)?;
    Ok(())
}

fn copied_source(dir: &Path, name: &str, fixture: &str) -> TestResult<PathBuf> {
    let path = dir.join(name);
    fs::copy(audio_fixture(fixture), &path)?;
    Ok(path)
}

fn assert_message(error: impl std::fmt::Display) {
    let message = error.to_string();
    assert!(
        !message.trim().is_empty(),
        "controlled failure must carry a readable error message"
    );
}

struct NightDrive {
    draft_path: PathBuf,
    output_path: PathBuf,
    source_paths: Vec<PathBuf>,
    artwork_path: PathBuf,
    draft_id: uuid::Uuid,
}

fn author_night_drive(dir: &Path) -> TestResult<NightDrive> {
    let draft_path = dir.join("Night Drive.vdraft");
    let output_path = dir.join("Night Drive.vdisc");

    let draft = DraftDisc::new("Night Drive")?;
    let draft_id = draft.id();
    save_draft(&draft_path, &draft)?;
    drop(draft);

    // A2/A3/A5/A6/A7: six local imports. Use distinct source paths and a
    // mixed-format matrix so the black-box run is not six aliases of one file.
    let fixture_names = [
        "valid.flac",
        "valid.mp3",
        "valid.opus",
        "valid.wav",
        "valid.flac",
        "valid.wav",
        "valid.opus",
    ];

    let mut source_paths = Vec::new();
    for (index, fixture) in fixture_names.iter().enumerate() {
        source_paths.push(copied_source(
            dir,
            &format!(
                "source-{:02}.{}",
                index + 1,
                fixture.split('.').next_back().unwrap()
            ),
            fixture,
        )?);
    }

    for source in &source_paths[..6] {
        import_track(&draft_path, source)?;
    }
    assert_eq!(load_draft(&draft_path)?.track_count(), 6);

    // A8: prove reorder + remove + refill before final review.
    let before_move = load_draft(&draft_path)?;
    let first_id = before_move.tracks()[0].id();
    drop(before_move);

    assert!(move_draft_track(&draft_path, 1, 6)?);
    let after_move = load_draft(&draft_path)?;
    assert_eq!(after_move.tracks()[5].id(), first_id);
    drop(after_move);

    let removed = remove_draft_track(&draft_path, 3)?;
    assert_eq!(load_draft(&draft_path)?.track_count(), 5);
    drop(removed);

    import_track(&draft_path, &source_paths[6])?;
    assert_eq!(load_draft(&draft_path)?.track_count(), 6);

    // C1/C2: select and customize the same draft.
    let artwork_path = dir.join("night-drive.png");
    image::RgbImage::from_pixel(16, 16, image::Rgb([23, 42, 77]))
        .save_with_format(&artwork_path, image::ImageFormat::Png)?;

    let customization = CustomizationSession::begin(&draft_path)?;
    customization.set_disc_color(DiscColor::new(18, 52, 86))?;
    customization.set_disc_image(&artwork_path)?;
    customization.set_disc_label("Night Drive")?;
    customization.set_disc_subtitle("Campaign 01")?;
    drop(customization);

    // A10: independent review/preflight.
    let preflight = run_preflight(&draft_path, &output_path);
    assert!(
        preflight.is_ready(),
        "Night Drive preflight failed: {:?}",
        preflight.issues()
    );

    Ok(NightDrive {
        draft_path,
        output_path,
        source_paths,
        artwork_path,
        draft_id,
    })
}

fn burn_and_verify_night_drive(flow: &NightDrive) -> TestResult<uuid::Uuid> {
    // A11.
    let burned = burn(&flow.draft_path, &flow.output_path)?;
    assert_eq!(burned.track_count, 6);
    let burned_id = burned.disc_id;
    drop(burned);

    // A12 format/read verification.
    let validated = validate_vdisc(&flow.output_path)?;
    assert_eq!(validated.manifest().title, "Night Drive");
    assert_eq!(validated.manifest().tracks.len(), 6);
    assert_eq!(
        validated.manifest().appearance.label.as_deref(),
        Some("Night Drive")
    );
    assert_eq!(
        validated.manifest().appearance.subtitle.as_deref(),
        Some("Campaign 01")
    );
    drop(validated);

    let reader = BurnedDisc::open(&flow.output_path)?;
    assert_eq!(reader.track_count(), 6);
    assert_eq!(reader.title(), "Night Drive");
    reader.verify_open_file()?;
    drop(reader);

    assert_ne!(burned_id, flow.draft_id);

    Ok(burned_id)
}

#[derive(Debug, Default)]
struct AuditBackend {
    opened: Arc<Mutex<Vec<usize>>>,
}

#[derive(Debug)]
struct AuditSession {
    finished: bool,
    gain: Mutex<f32>,
}

impl PlaybackSession for AuditSession {
    fn set_gain(&self, gain: f32) -> PlaybackResult<()> {
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return Err(vdisc_core::PlaybackError::BackendInvariant(
                "invalid audit gain",
            ));
        }
        *self.gain.lock().unwrap() = gain;
        Ok(())
    }

    fn play(&self) -> PlaybackResult<()> {
        Ok(())
    }

    fn pause(&self) -> PlaybackResult<()> {
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        0
    }

    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        if self.finished {
            None
        } else {
            self.finished = true;
            Some(PlaybackBackendEvent::TrackFinished)
        }
    }
}

impl PlaybackBackend for AuditBackend {
    type Session = AuditSession;

    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track_index: usize,
        _position_ms: u64,
    ) -> PlaybackResult<Self::Session> {
        let mut payload = disc
            .track_payload(track_index)?
            .expect("manifest track must have embedded payload");
        let mut probe = [0u8; 32];
        let read = payload
            .read(&mut probe)
            .map_err(|error| vdisc_core::PlaybackError::Decode(error.to_string()))?;
        assert!(read > 0, "embedded track payload must not be empty");

        self.opened.lock().unwrap().push(track_index);

        Ok(AuditSession {
            finished: false,
            gain: Mutex::new(1.0),
        })
    }
}

fn audit_play_all_six(path: &Path) -> TestResult {
    let opened = Arc::new(Mutex::new(Vec::new()));
    let backend = AuditBackend {
        opened: Arc::clone(&opened),
    };
    let mut player = PlaybackPlayer::with_backend(backend);

    player.insert(path)?;
    assert_eq!(player.track_count(), 6);
    assert_eq!(player.current_track_index(), Some(0));

    player.play_to_end()?;

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(*opened.lock().unwrap(), vec![0, 1, 2, 3, 4, 5]);

    Ok(())
}

fn fresh_second_cd(path: &Path, first_draft_id: uuid::Uuid) -> TestResult {
    let second = DraftDisc::new("Second CD")?;
    assert_ne!(second.id(), first_draft_id);
    assert_eq!(second.track_count(), 0);
    assert_eq!(*second.appearance().base_color(), DiscColor::WHITE);
    assert!(second.appearance().image().is_none());
    assert!(second.appearance().label().is_none());
    assert!(second.appearance().subtitle().is_none());

    save_draft(path, &second)?;
    let reopened = load_draft(path)?;
    assert_eq!(reopened.id(), second.id());
    assert_eq!(reopened.track_count(), 0);

    Ok(())
}

#[test]
fn complete_supported_algorithm_survives_restart_and_starts_second_cd() -> TestResult {
    if env::var_os(RESTART_CHILD).is_some() {
        let disc = env::var_os(RESTART_DISC).ok_or("restart child missing VDISC path")?;
        audit_play_all_six(Path::new(&disc))?;
        return Ok(());
    }

    let dir = tempfile::tempdir()?;
    let flow = author_night_drive(dir.path())?;
    let _burned_id = burn_and_verify_night_drive(&flow)?;

    // The finished artifact must stand alone for A12.
    fs::remove_file(&flow.draft_path)?;
    for source in &flow.source_paths {
        fs::remove_file(source)?;
    }
    fs::remove_file(&flow.artwork_path)?;

    audit_play_all_six(&flow.output_path)?;

    // Explicit fresh-process reopen/play.
    let status = Command::new(env::current_exe()?)
        .args([
            "--exact",
            "complete_supported_algorithm_survives_restart_and_starts_second_cd",
        ])
        .env(RESTART_CHILD, "1")
        .env(RESTART_DISC, &flow.output_path)
        .status()?;
    assert!(status.success(), "fresh-process VDISC reopen/play failed");

    // A13.
    fresh_second_cd(&dir.path().join("second.vdraft"), flow.draft_id)?;

    // Record Campaign 01's intentionally skipped branches in the executable
    // audit itself, so they cannot silently look like omissions.
    let explicitly_deferred = [
        "A4 Spotify search/select",
        "C3 CD case",
        "C4+ undefined customization",
    ];
    assert_eq!(explicitly_deferred.len(), 3);

    Ok(())
}

fn ready_one_track_draft(dir: &Path, stem: &str) -> TestResult<(PathBuf, PathBuf)> {
    let source = copied_source(dir, &format!("{stem}.flac"), "valid.flac")?;
    let draft_path = dir.join(format!("{stem}.vdraft"));
    save_draft(&draft_path, &DraftDisc::new(stem)?)?;
    import_track(&draft_path, &source)?;
    Ok((draft_path, source))
}

#[test]
fn destructive_campaign_matrix_fails_cleanly() -> TestResult {
    // 1. Corrupt archive: truncate an otherwise valid fixture.
    let dir = tempfile::tempdir()?;
    let corrupt_archive = dir.path().join("truncated.vdisc");
    let mut bytes = fs::read(vdisc_fixture("valid-v1.vdisc"))?;
    bytes.truncate(bytes.len().saturating_sub(17));
    fs::write(&corrupt_archive, bytes)?;
    let error = validate_vdisc(&corrupt_archive).unwrap_err();
    assert_message(error);

    // 2. Remove required archive entry.
    let error = validate_vdisc(vdisc_fixture("missing-manifest.vdisc")).unwrap_err();
    assert_message(error);

    // 3. Alter embedded audio payload.
    let error = validate_vdisc(vdisc_fixture("corrupt-payload.vdisc")).unwrap_err();
    assert_message(error);

    // 4. Malform manifest.
    let error = validate_vdisc(vdisc_fixture("malformed-manifest.vdisc")).unwrap_err();
    assert_message(error);

    // 5. Unsupported format version.
    let error = validate_vdisc(vdisc_fixture("future-version.vdisc")).unwrap_err();
    assert_message(error);

    // 6. Unreadable/read-only output destination.
    let readonly_parent = dir.path().join("readonly-output");
    fs::create_dir(&readonly_parent)?;
    let original_mode = fs::metadata(&readonly_parent)?.permissions().mode();
    let mut permissions = fs::metadata(&readonly_parent)?.permissions();
    permissions.set_mode(original_mode & !0o222);
    fs::set_permissions(&readonly_parent, permissions)?;

    let (readonly_draft, _) = ready_one_track_draft(dir.path(), "readonly")?;
    let readonly_output = readonly_parent.join("disc.vdisc");
    let report = run_preflight(&readonly_draft, &readonly_output);
    assert!(report.has_issue(PreflightIssueCode::OutputParentReadOnly));
    let error = burn(&readonly_draft, &readonly_output).unwrap_err();
    assert_message(error);

    let mut permissions = fs::metadata(&readonly_parent)?.permissions();
    permissions.set_mode(original_mode);
    fs::set_permissions(&readonly_parent, permissions)?;

    // 7. Delete source before burn.
    let (deleted_draft, deleted_source) = ready_one_track_draft(dir.path(), "deleted-source")?;
    fs::remove_file(&deleted_source)?;
    let deleted_output = dir.path().join("deleted-source.vdisc");
    let report = run_preflight(&deleted_draft, &deleted_output);
    assert!(report.has_issue(PreflightIssueCode::SourceMissing));
    let error = burn(&deleted_draft, &deleted_output).unwrap_err();
    assert_message(error);
    assert!(!deleted_output.exists());

    // 8. Modify source before burn.
    let (modified_draft, modified_source) = ready_one_track_draft(dir.path(), "modified-source")?;
    let mut modified_bytes = fs::read(&modified_source)?;
    modified_bytes.push(0);
    fs::write(&modified_source, modified_bytes)?;
    let modified_output = dir.path().join("modified-source.vdisc");
    let report = run_preflight(&modified_draft, &modified_output);
    assert!(report.has_issue(PreflightIssueCode::SourceChanged));
    let error = burn(&modified_draft, &modified_output).unwrap_err();
    assert_message(error);
    assert!(!modified_output.exists());

    // 9. Attempt seventh track.
    let seventh_draft = dir.path().join("seventh.vdraft");
    save_draft(&seventh_draft, &DraftDisc::new("Six Only")?)?;
    let seventh_source = copied_source(dir.path(), "seventh-source.flac", "valid.flac")?;
    for _ in 0..6 {
        import_track(&seventh_draft, &seventh_source)?;
    }
    let selected = AddTrackRequest::begin(&seventh_draft)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&seventh_source)?;
    let seventh = ValidatedLocalAudio::validate(selected)?;
    let error = add_validated_local_track(seventh).unwrap_err();
    assert_message(&error);
    assert!(matches!(error, VdiscError::DiscFull { capacity: 6 }));
    assert_eq!(load_draft(&seventh_draft)?.track_count(), 6);

    // 10. Attempt mutation after burn.
    let (burned_draft, _) = ready_one_track_draft(dir.path(), "immutable")?;
    let burned_path = dir.path().join("immutable.vdisc");
    let _ = burn(&burned_draft, &burned_path)?;

    let error = AddTrackRequest::begin(&burned_path).unwrap_err();
    assert_message(error);
    let error = CustomizationSession::begin(&burned_path).unwrap_err();
    assert_message(error);
    let replacement = DraftDisc::new("Must Not Replace Burned Disc")?;
    let error = save_draft(&burned_path, &replacement).unwrap_err();
    assert_message(error);
    validate_vdisc(&burned_path)?;

    Ok(())
}

#[test]
#[ignore = "requires a real Linux audio output device"]
fn real_linux_player_plays_six_tracks_and_reopens_after_process_restart() -> TestResult {
    if env::var_os(HARDWARE_CHILD).is_some() {
        let disc = env::var_os(HARDWARE_DISC).ok_or("hardware child missing VDISC path")?;
        let mut player = LinuxAudioPlayer::new();
        player.insert(Path::new(&disc))?;
        player.play_to_end()?;
        assert_eq!(player.state(), PlayerState::Stopped);
        return Ok(());
    }

    let dir = tempfile::tempdir()?;
    let flow = author_night_drive(dir.path())?;
    burn_and_verify_night_drive(&flow)?;

    fs::remove_file(&flow.draft_path)?;

    let mut player = LinuxAudioPlayer::new();
    player.insert(&flow.output_path)?;
    player.play_to_end()?;
    assert_eq!(player.state(), PlayerState::Stopped);
    drop(player);

    let status = Command::new(env::current_exe()?)
        .args([
            "--exact",
            "real_linux_player_plays_six_tracks_and_reopens_after_process_restart",
            "--ignored",
            "--nocapture",
        ])
        .env(HARDWARE_CHILD, "1")
        .env(HARDWARE_DISC, &flow.output_path)
        .status()?;
    assert!(status.success(), "fresh-process real Linux playback failed");

    fresh_second_cd(
        &dir.path().join("second-after-hardware.vdraft"),
        flow.draft_id,
    )?;

    Ok(())
}
