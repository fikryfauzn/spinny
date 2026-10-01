#![cfg(target_os = "linux")]

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use vdisc_core::{
    AddTrackRequest, CustomizationSession, DiscColor, DraftDisc, TrackSourceKind,
    ValidatedLocalAudio, add_validated_local_track, burn, format::validate_vdisc, load_draft,
    save_draft,
};

type TestResult<T = ()> = std::result::Result<T, Box<dyn Error>>;

const RESTART_CHILD: &str = "VDISC_OBJECTIVE18_RESTART_CHILD";
const RESTART_DRAFT_B: &str = "VDISC_OBJECTIVE18_DRAFT_B";

fn audio_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio/valid.flac")
}

fn add_fixture_track(draft_path: &Path) -> TestResult<()> {
    let selected = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(audio_fixture())?;

    let validated = ValidatedLocalAudio::validate(selected)?;
    add_validated_local_track(validated)?;

    Ok(())
}

fn burn_disc_a(dir: &Path) -> TestResult<(PathBuf, uuid::Uuid)> {
    let draft_path = dir.join("disc-a.vdraft");
    let output_path = dir.join("disc-a.vdisc");

    let draft = DraftDisc::new("Disc A")?;
    let draft_id = draft.id();
    save_draft(&draft_path, &draft)?;
    drop(draft);

    add_fixture_track(&draft_path)?;

    let customization = CustomizationSession::begin(&draft_path)?;
    customization.set_disc_color(DiscColor::new(12, 34, 56))?;
    customization.set_disc_label("Disc A Label")?;
    customization.set_disc_subtitle("Disc A Subtitle")?;
    drop(customization);

    let result = burn(&draft_path, &output_path)?;
    assert_eq!(result.track_count, 1);
    assert_eq!(result.output_path, output_path);
    drop(result);

    // Finishing A must release the authoring side. Removing the draft proves
    // the finished artifact is not held together by a live draft/session.
    fs::remove_file(&draft_path)?;

    validate_vdisc(&output_path)?;

    Ok((output_path, draft_id))
}

#[test]
fn burn_a_then_create_b_has_no_lifecycle_leakage() -> TestResult<()> {
    let dir = tempfile::tempdir()?;

    let (disc_a_path, draft_a_id) = burn_disc_a(dir.path())?;
    let disc_a_before = fs::read(&disc_a_path)?;

    let draft_b_path = dir.path().join("disc-b.vdraft");
    let draft_b = DraftDisc::new("Disc B")?;

    assert_ne!(draft_b.id(), draft_a_id);
    assert_eq!(draft_b.title(), "Disc B");
    assert_eq!(draft_b.track_count(), 0);
    assert_eq!(*draft_b.appearance().base_color(), DiscColor::WHITE);
    assert!(draft_b.appearance().image().is_none());
    assert!(draft_b.appearance().label().is_none());
    assert!(draft_b.appearance().subtitle().is_none());

    save_draft(&draft_b_path, &draft_b)?;
    let draft_b_id = draft_b.id();
    drop(draft_b);

    let reopened_b = load_draft(&draft_b_path)?;
    assert_eq!(reopened_b.id(), draft_b_id);
    assert_ne!(reopened_b.id(), draft_a_id);
    assert_eq!(reopened_b.track_count(), 0);
    assert_eq!(*reopened_b.appearance().base_color(), DiscColor::WHITE);
    assert!(reopened_b.appearance().image().is_none());
    assert!(reopened_b.appearance().label().is_none());
    assert!(reopened_b.appearance().subtitle().is_none());
    drop(reopened_b);

    // Actively author B after the fresh-state assertions. A must remain
    // byte-identical and valid throughout unrelated authoring work.
    add_fixture_track(&draft_b_path)?;
    let customization_b = CustomizationSession::begin(&draft_b_path)?;
    customization_b.set_disc_color(DiscColor::new(200, 100, 50))?;
    customization_b.set_disc_label("Disc B Label")?;
    drop(customization_b);

    assert_eq!(fs::read(&disc_a_path)?, disc_a_before);
    validate_vdisc(&disc_a_path)?;

    Ok(())
}

#[test]
fn fresh_second_draft_can_start_after_process_restart() -> TestResult<()> {
    if env::var_os(RESTART_CHILD).is_some() {
        let draft_b_path =
            env::var_os(RESTART_DRAFT_B).ok_or("restart child is missing second-draft path")?;

        let draft_b = DraftDisc::new("Disc B After Restart")?;
        save_draft(PathBuf::from(draft_b_path), &draft_b)?;

        return Ok(());
    }

    let dir = tempfile::tempdir()?;
    let (disc_a_path, draft_a_id) = burn_disc_a(dir.path())?;
    let disc_a_before = fs::read(&disc_a_path)?;
    let draft_b_path = dir.path().join("disc-b-after-restart.vdraft");

    let status = Command::new(env::current_exe()?)
        .args([
            "--exact",
            "fresh_second_draft_can_start_after_process_restart",
        ])
        .env(RESTART_CHILD, "1")
        .env(RESTART_DRAFT_B, &draft_b_path)
        .status()?;

    assert!(status.success(), "restart child process failed");

    let draft_b = load_draft(&draft_b_path)?;
    assert_ne!(draft_b.id(), draft_a_id);
    assert_eq!(draft_b.title(), "Disc B After Restart");
    assert_eq!(draft_b.track_count(), 0);
    assert_eq!(*draft_b.appearance().base_color(), DiscColor::WHITE);
    assert!(draft_b.appearance().image().is_none());
    assert!(draft_b.appearance().label().is_none());
    assert!(draft_b.appearance().subtitle().is_none());

    assert_eq!(fs::read(&disc_a_path)?, disc_a_before);
    validate_vdisc(&disc_a_path)?;

    Ok(())
}
