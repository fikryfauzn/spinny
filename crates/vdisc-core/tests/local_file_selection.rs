mod common;

use std::{error::Error, fs};

use vdisc_core::{AddTrackRequest, DraftDisc, TrackSourceKind, VdiscError, save_draft};

#[test]
fn local_source_can_select_existing_file() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("night-drive.vdraft");

    let audio_path = sandbox.path().join("song.flac");

    let draft = DraftDisc::new("Night Drive")?;
    let expected_id = draft.id();

    save_draft(&draft_path, &draft)?;

    fs::write(&audio_path, b"placeholder audio bytes")?;

    let request = AddTrackRequest::begin(&draft_path)?;

    let source = request.select_source(TrackSourceKind::Local);

    let selection = source.select_local_file(&audio_path)?;

    assert_eq!(selection.target_disc_id(), expected_id);

    assert_eq!(selection.target_path(), draft_path);

    assert_eq!(selection.source_path(), fs::canonicalize(audio_path)?);

    Ok(())
}

#[test]
fn selected_path_is_canonical() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = sandbox.path().join("song.opus");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&source_path, b"placeholder")?;

    let selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source_path)?;

    assert!(selection.source_path().is_absolute());

    assert_eq!(selection.source_path(), fs::canonicalize(source_path)?);

    Ok(())
}

#[test]
fn missing_local_file_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let missing = sandbox.path().join("missing.flac");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let result = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&missing);

    assert!(matches!(result, Err(VdiscError::LocalFileNotFound { .. })));

    Ok(())
}

#[test]
fn directory_is_rejected_as_local_file() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let directory = sandbox.path().join("music");

    fs::create_dir(&directory)?;

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let result = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&directory);

    assert!(matches!(
        result,
        Err(VdiscError::LocalFileNotRegular { .. })
    ));

    Ok(())
}

#[test]
fn spotify_source_cannot_select_local_file() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = sandbox.path().join("song.flac");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&source_path, b"placeholder")?;

    let result = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Spotify)
        .select_local_file(&source_path);

    assert!(matches!(
        result,
        Err(VdiscError::LocalFileRequiresLocalSource)
    ));

    Ok(())
}

#[test]
fn selecting_local_file_does_not_modify_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = sandbox.path().join("song.flac");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let original = b"do not modify me";

    fs::write(&source_path, original)?;

    let before = fs::read(&source_path)?;

    let _selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source_path)?;

    let after = fs::read(&source_path)?;

    assert_eq!(before, after);
    assert_eq!(after, original);

    Ok(())
}

#[test]
fn selecting_local_file_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = sandbox.path().join("song.flac");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&source_path, b"placeholder")?;

    let before = fs::read(&draft_path)?;

    let _selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source_path)?;

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn selection_does_not_require_audio_extension() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = sandbox.path().join("mystery-file");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    fs::write(&source_path, b"not yet validated as audio")?;

    let selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source_path)?;

    assert_eq!(selection.source_path(), fs::canonicalize(source_path)?);

    Ok(())
}
