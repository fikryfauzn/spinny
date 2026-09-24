mod common;

use std::{
    error::Error,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use vdisc_core::{
    AddTrackRequest, DraftDisc, TrackSourceKind, ValidatedLocalAudio, VdiscError,
    add_validated_local_track, load_draft, save_draft, verify_draft_source_integrity,
};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio/valid.flac")
}

fn add_source(draft_path: &Path, source_path: &Path) -> Result<(), Box<dyn Error>> {
    let selection = AddTrackRequest::begin(draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(source_path)?;

    let audio = ValidatedLocalAudio::validate(selection)?;

    add_validated_local_track(audio)?;

    Ok(())
}

fn create_source_copy(directory: &Path, name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let destination = directory.join(name);

    fs::copy(fixture_path(), &destination)?;

    Ok(destination)
}

#[test]
fn imported_track_has_fingerprint() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    let draft = load_draft(&draft_path)?;

    let fingerprint = draft.tracks()[0]
        .source_fingerprint()
        .expect("new tracks must have fingerprints");

    assert_eq!(fingerprint.size_bytes(), fs::metadata(&source_path)?.len());

    assert_eq!(fingerprint.sha256().len(), 64);

    Ok(())
}

#[test]
fn unchanged_source_passes() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    verify_draft_source_integrity(&draft_path)?;

    Ok(())
}

#[test]
fn deleted_source_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    fs::remove_file(&source_path)?;

    let result = verify_draft_source_integrity(&draft_path);

    assert!(matches!(result, Err(VdiscError::SourceMissing { .. })));

    Ok(())
}

#[test]
fn moved_source_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    let moved_path = sandbox.path().join("moved.flac");

    fs::rename(&source_path, &moved_path)?;

    let result = verify_draft_source_integrity(&draft_path);

    assert!(matches!(result, Err(VdiscError::SourceMissing { .. })));

    Ok(())
}

#[test]
fn modified_source_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    let mut file = fs::OpenOptions::new().append(true).open(&source_path)?;

    file.write_all(b"changed")?;

    let result = verify_draft_source_integrity(&draft_path);

    assert!(matches!(result, Err(VdiscError::SourceChanged { .. })));

    Ok(())
}

#[test]
fn same_size_modification_is_detected_by_hash() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    let original_size = fs::metadata(&source_path)?.len();

    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&source_path)?;

    file.seek(SeekFrom::Start(64))?;

    let mut byte = [0_u8; 1];

    file.read_exact(&mut byte)?;

    byte[0] ^= 0xff;

    file.seek(SeekFrom::Start(64))?;

    file.write_all(&byte)?;

    drop(file);

    assert_eq!(fs::metadata(&source_path,)?.len(), original_size);

    let result = verify_draft_source_integrity(&draft_path);

    assert!(matches!(result, Err(VdiscError::SourceChanged { .. })));

    Ok(())
}

#[test]
fn one_bad_track_blocks_entire_integrity_gate() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let first = create_source_copy(sandbox.path(), "first.flac")?;

    let second = create_source_copy(sandbox.path(), "second.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &first)?;

    add_source(&draft_path, &second)?;

    fs::remove_file(&second)?;

    let result = verify_draft_source_integrity(&draft_path);

    assert!(result.is_err());

    Ok(())
}

#[test]
fn failed_integrity_check_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    let source_path = create_source_copy(sandbox.path(), "song.flac")?;

    save_draft(&draft_path, &DraftDisc::new("Disc")?)?;

    add_source(&draft_path, &source_path)?;

    let before = fs::read(&draft_path)?;

    fs::remove_file(&source_path)?;

    let result = verify_draft_source_integrity(&draft_path);

    assert!(result.is_err());

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}
