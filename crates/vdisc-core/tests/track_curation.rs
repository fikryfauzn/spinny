mod common;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackSourceKind, ValidatedLocalAudio, VdiscError,
    add_validated_local_track, load_draft, move_draft_track, remove_draft_track, save_draft,
};

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/audio")
        .join(file_name)
}

fn add_fixture_track(draft_path: &Path) -> Result<Uuid, Box<dyn Error>> {
    let source_path = fixture_path("valid.flac");

    let selection = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(source_path)?;

    let audio = ValidatedLocalAudio::validate(selection)?;

    let track = add_validated_local_track(audio)?;

    Ok(track.id())
}

fn create_draft_with_tracks(draft_path: &Path, count: usize) -> Result<Vec<Uuid>, Box<dyn Error>> {
    let draft = DraftDisc::new("Curation Test")?;

    save_draft(draft_path, &draft)?;

    let mut ids = Vec::with_capacity(count);

    for _ in 0..count {
        ids.push(add_fixture_track(draft_path)?);
    }

    Ok(ids)
}

#[test]
fn remove_first_track() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 3)?;

    let removed = remove_draft_track(&path, 1)?;

    assert_eq!(removed.id(), ids[0]);

    let reopened = load_draft(&path)?;

    assert_eq!(reopened.track_count(), 2);

    assert_eq!(reopened.tracks()[0].id(), ids[1]);

    assert_eq!(reopened.tracks()[1].id(), ids[2]);

    Ok(())
}

#[test]
fn remove_middle_track() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 3)?;

    let removed = remove_draft_track(&path, 2)?;

    assert_eq!(removed.id(), ids[1]);

    let reopened = load_draft(&path)?;

    assert_eq!(reopened.tracks()[0].id(), ids[0]);

    assert_eq!(reopened.tracks()[1].id(), ids[2]);

    Ok(())
}

#[test]
fn remove_last_track() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 3)?;

    let removed = remove_draft_track(&path, 3)?;

    assert_eq!(removed.id(), ids[2]);

    let reopened = load_draft(&path)?;

    assert_eq!(reopened.track_count(), 2);

    Ok(())
}

#[test]
fn removed_slot_can_be_used_again() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 6)?;

    let full = load_draft(&path)?;

    assert!(full.is_full());

    remove_draft_track(&path, 3)?;

    let after_remove = load_draft(&path)?;

    assert_eq!(after_remove.track_count(), 5);

    assert!(!after_remove.is_full());

    add_fixture_track(&path)?;

    let final_draft = load_draft(&path)?;

    assert_eq!(final_draft.track_count(), 6);

    assert!(final_draft.is_full());

    Ok(())
}

#[test]
fn move_last_track_to_second() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 5)?;

    let changed = move_draft_track(&path, 5, 2)?;

    assert!(changed);

    let reopened = load_draft(&path)?;

    let actual: Vec<Uuid> = reopened.tracks().iter().map(|track| track.id()).collect();

    assert_eq!(actual, vec![ids[0], ids[4], ids[1], ids[2], ids[3],]);

    Ok(())
}

#[test]
fn move_second_track_to_last() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 5)?;

    move_draft_track(&path, 2, 5)?;

    let reopened = load_draft(&path)?;

    let actual: Vec<Uuid> = reopened.tracks().iter().map(|track| track.id()).collect();

    assert_eq!(actual, vec![ids[0], ids[2], ids[3], ids[4], ids[1],]);

    Ok(())
}

#[test]
fn move_first_track_to_last() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let ids = create_draft_with_tracks(&path, 4)?;

    move_draft_track(&path, 1, 4)?;

    let reopened = load_draft(&path)?;

    let actual: Vec<Uuid> = reopened.tracks().iter().map(|track| track.id()).collect();

    assert_eq!(actual, vec![ids[1], ids[2], ids[3], ids[0],]);

    Ok(())
}

#[test]
fn moving_track_to_same_position_is_noop() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 3)?;

    let before = fs::read(&path)?;

    let changed = move_draft_track(&path, 2, 2)?;

    let after = fs::read(&path)?;

    assert!(!changed);
    assert_eq!(before, after);

    Ok(())
}

#[test]
fn remove_position_zero_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 3)?;

    let before = fs::read(&path)?;

    let result = remove_draft_track(&path, 0);

    assert!(matches!(
        result,
        Err(VdiscError::TrackPositionOutOfBounds {
            position: 0,
            track_count: 3,
        })
    ));

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn remove_position_above_track_count_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 3)?;

    let before = fs::read(&path)?;

    let result = remove_draft_track(&path, 4);

    assert!(matches!(
        result,
        Err(VdiscError::TrackPositionOutOfBounds {
            position: 4,
            track_count: 3,
        })
    ));

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn invalid_move_source_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 3)?;

    let before = fs::read(&path)?;

    let result = move_draft_track(&path, 4, 1);

    assert!(matches!(
        result,
        Err(VdiscError::TrackPositionOutOfBounds { .. })
    ));

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn invalid_move_destination_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    create_draft_with_tracks(&path, 3)?;

    let before = fs::read(&path)?;

    let result = move_draft_track(&path, 1, 4);

    assert!(matches!(
        result,
        Err(VdiscError::TrackPositionOutOfBounds { .. })
    ));

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}
