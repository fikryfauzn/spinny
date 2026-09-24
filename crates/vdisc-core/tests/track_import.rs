mod common;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackSourceKind, ValidatedLocalAudio, VdiscError,
    add_validated_local_track, load_draft, save_draft,
};

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/audio")
        .join(file_name)
}

fn validated_audio(
    draft_path: &Path,
    file_name: &str,
) -> Result<ValidatedLocalAudio, Box<dyn Error>> {
    let source_path = fixture_path(file_name);

    let selection = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(source_path)?;

    Ok(ValidatedLocalAudio::validate(selection)?)
}

#[test]
fn validated_track_is_added_to_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    let added = add_validated_local_track(audio)?;

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.track_count(), 1);

    assert_eq!(reopened.tracks()[0].id(), added.id());

    Ok(())
}

#[test]
fn technical_audio_data_is_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    add_validated_local_track(audio)?;

    let reopened = load_draft(&draft_path)?;

    let track = &reopened.tracks()[0];

    assert_eq!(track.codec(), "flac");

    assert_eq!(track.sample_rate(), Some(48_000));

    assert_eq!(track.channels(), Some(2));

    assert!(track.duration_ms().is_some());

    Ok(())
}

#[test]
fn metadata_is_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    add_validated_local_track(audio)?;

    let reopened = load_draft(&draft_path)?;

    let track = &reopened.tracks()[0];

    assert_eq!(track.title(), Some("VDISC Matrix Fixture"));

    assert_eq!(track.artist(), Some("VDISC Test Artist"));

    assert_eq!(track.album(), Some("VDISC Compatibility Matrix"));

    Ok(())
}

#[test]
fn source_path_is_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let expected_source = fs::canonicalize(fixture_path("valid.flac"))?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    add_validated_local_track(audio)?;

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.tracks()[0].source_path(), expected_source);

    Ok(())
}

#[test]
fn duplicate_source_files_are_allowed() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    for _ in 0..2 {
        let audio = validated_audio(&draft_path, "valid.flac")?;

        add_validated_local_track(audio)?;
    }

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.track_count(), 2);

    assert_ne!(reopened.tracks()[0].id(), reopened.tracks()[1].id());

    assert_eq!(
        reopened.tracks()[0].source_path(),
        reopened.tracks()[1].source_path()
    );

    Ok(())
}

#[test]
fn six_tracks_fill_the_disc() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    for _ in 0..6 {
        let audio = validated_audio(&draft_path, "valid.flac")?;

        add_validated_local_track(audio)?;
    }

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.track_count(), 6);

    assert!(reopened.is_full());

    Ok(())
}

#[test]
fn seventh_track_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    for _ in 0..6 {
        let audio = validated_audio(&draft_path, "valid.flac")?;

        add_validated_local_track(audio)?;
    }

    let before = fs::read(&draft_path)?;

    let seventh = validated_audio(&draft_path, "valid.flac")?;

    let result = add_validated_local_track(seventh);

    assert!(matches!(result, Err(VdiscError::DiscFull { capacity: 6 })));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.track_count(), 6);

    Ok(())
}

#[test]
fn replaced_draft_at_same_path_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let original = DraftDisc::new("Original")?;

    save_draft(&draft_path, &original)?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    /*
     * Replace the file with another valid draft
     * after audio validation.
     */
    let replacement = DraftDisc::new("Replacement")?;

    save_draft(&draft_path, &replacement)?;

    let before = fs::read(&draft_path)?;

    let result = add_validated_local_track(audio);

    assert!(matches!(
        result,
        Err(VdiscError::DraftIdentityMismatch { .. })
    ));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.id(), replacement.id());

    assert_eq!(reopened.track_count(), 0);

    Ok(())
}

#[test]
fn adding_track_never_modifies_source_audio() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Disc")?;

    save_draft(&draft_path, &draft)?;

    let source_path = fixture_path("valid.flac");

    let before = fs::read(&source_path)?;

    let audio = validated_audio(&draft_path, "valid.flac")?;

    add_validated_local_track(audio)?;

    let after = fs::read(&source_path)?;

    assert_eq!(before, after);

    Ok(())
}
