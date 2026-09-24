mod common;

use std::{error::Error, fs};

use vdisc_core::{AddTrackRequest, DraftDisc, TrackSourceKind, save_draft};

#[test]
fn can_select_local_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("night-drive.vdraft");

    let draft = DraftDisc::new("Night Drive")?;
    let expected_id = draft.id();

    save_draft(&path, &draft)?;

    let request = AddTrackRequest::begin(&path)?;
    let selection = request.select_source(TrackSourceKind::Local);

    assert_eq!(selection.source(), TrackSourceKind::Local);
    assert_eq!(selection.target_path(), path);
    assert_eq!(selection.target_disc_id(), expected_id);

    Ok(())
}

#[test]
fn can_represent_spotify_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("night-drive.vdraft");

    let draft = DraftDisc::new("Night Drive")?;
    let expected_id = draft.id();

    save_draft(&path, &draft)?;

    let request = AddTrackRequest::begin(&path)?;
    let selection = request.select_source(TrackSourceKind::Spotify);

    assert_eq!(selection.source(), TrackSourceKind::Spotify);
    assert_eq!(selection.target_disc_id(), expected_id);

    Ok(())
}

#[test]
fn selecting_source_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("night-drive.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&path, &draft)?;

    let before = fs::read(&path)?;

    let request = AddTrackRequest::begin(&path)?;
    let _selection = request.select_source(TrackSourceKind::Local);

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn local_and_spotify_are_distinct_sources() {
    assert_ne!(TrackSourceKind::Local, TrackSourceKind::Spotify);
}

#[test]
fn source_selection_preserves_target_path() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&path, &draft)?;

    let request = AddTrackRequest::begin(&path)?;
    let selection = request.select_source(TrackSourceKind::Local);

    assert_eq!(selection.target_path(), path);

    Ok(())
}
