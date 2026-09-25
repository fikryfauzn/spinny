mod common;

use std::{
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use image::{ImageFormat, Rgb, RgbImage};
use serde_json::Value;

use vdisc_core::{
    AddTrackRequest, CustomizationSession, DraftDisc, PreflightIssueCode, TrackSourceKind,
    ValidatedLocalAudio, add_validated_local_track, run_preflight, save_draft,
};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio/valid.flac")
}

fn create_source_copy(directory: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let source = directory.join("song.flac");

    fs::copy(fixture_path(), &source)?;

    Ok(source)
}

fn create_ready_draft(directory: &Path) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let draft_path = directory.join("disc.vdraft");

    let source_path = create_source_copy(directory)?;

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&draft_path, &draft)?;

    let selection = AddTrackRequest::begin(&draft_path)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source_path)?;

    let audio = ValidatedLocalAudio::validate(selection)?;

    add_validated_local_track(audio)?;

    Ok((draft_path, source_path))
}

fn create_png(path: &Path) -> Result<(), Box<dyn Error>> {
    let image = RgbImage::from_pixel(12, 12, Rgb([30, 60, 90]));

    image.save_with_format(path, ImageFormat::Png)?;

    Ok(())
}

#[test]
fn valid_draft_is_ready_to_burn() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let output = sandbox.path().join("Night Drive.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(
        report.is_ready(),
        "unexpected preflight issues: {:?}",
        report.issues(),
    );

    Ok(())
}

#[test]
fn empty_disc_is_not_ready() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = sandbox.path().join("disc.vdraft");

    save_draft(&draft_path, &DraftDisc::new("Empty Disc")?)?;

    let output = sandbox.path().join("empty.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::TrackCountInvalid));

    Ok(())
}

#[test]
fn missing_source_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, source_path) = create_ready_draft(sandbox.path())?;

    fs::remove_file(source_path)?;

    let output = sandbox.path().join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::SourceMissing));

    assert!(!report.is_ready());

    Ok(())
}

#[test]
fn modified_source_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, source_path) = create_ready_draft(sandbox.path())?;

    let mut file = fs::OpenOptions::new().append(true).open(&source_path)?;

    file.write_all(b"changed")?;

    let output = sandbox.path().join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::SourceChanged));

    assert!(!report.is_ready());

    Ok(())
}

#[test]
fn changed_artwork_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let artwork = sandbox.path().join("art.png");

    create_png(&artwork)?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_image(&artwork)?;

    let mut file = fs::OpenOptions::new().append(true).open(&artwork)?;

    file.write_all(b"changed")?;

    let output = sandbox.path().join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::ArtworkInvalid));

    Ok(())
}

#[test]
fn invalid_output_extension_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let output = sandbox.path().join("disc.zip");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::OutputExtensionInvalid));

    Ok(())
}

#[test]
fn missing_output_parent_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let output = sandbox.path().join("missing").join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::OutputParentMissing));

    Ok(())
}

#[test]
fn existing_output_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let output = sandbox.path().join("disc.vdisc");

    fs::write(&output, b"existing")?;

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::OutputAlreadyExists));

    Ok(())
}

#[test]
fn malformed_stored_metadata_blocks_preflight() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, _source_path) = create_ready_draft(sandbox.path())?;

    let bytes = fs::read(&draft_path)?;

    let mut value: Value = serde_json::from_slice(&bytes)?;

    value["tracks"][0]["title"] = Value::String("   ".to_string());

    fs::write(&draft_path, serde_json::to_vec_pretty(&value)?)?;

    let output = sandbox.path().join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::MetadataInvalid));

    Ok(())
}

#[test]
fn preflight_collects_multiple_independent_problems() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, source_path) = create_ready_draft(sandbox.path())?;

    fs::remove_file(source_path)?;

    let output = sandbox.path().join("disc.invalid");

    let report = run_preflight(&draft_path, &output);

    assert!(report.has_issue(PreflightIssueCode::SourceMissing));

    assert!(report.has_issue(PreflightIssueCode::OutputExtensionInvalid));

    assert!(report.issues().len() >= 2);

    Ok(())
}

#[test]
fn preflight_never_mutates_draft_or_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let (draft_path, source_path) = create_ready_draft(sandbox.path())?;

    let draft_before = fs::read(&draft_path)?;

    let source_before = fs::read(&source_path)?;

    let output = sandbox.path().join("disc.vdisc");

    let report = run_preflight(&draft_path, &output);

    assert!(report.is_ready());

    let draft_after = fs::read(&draft_path)?;

    let source_after = fs::read(&source_path)?;

    assert_eq!(draft_before, draft_after);

    assert_eq!(source_before, source_after);

    assert!(!output.exists());

    Ok(())
}
