mod common;

use std::{error::Error, fs};

use vdisc_core::{AddTrackRequest, DraftDisc, VdiscError, save_draft};

#[test]
fn valid_draft_can_receive_add_request() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("night-drive.vdraft");

    let draft = DraftDisc::new("Night Drive")?;
    let expected_id = draft.id();

    save_draft(&path, &draft)?;

    let request = AddTrackRequest::begin(&path)?;

    assert_eq!(request.target_path(), path);
    assert_eq!(request.target_disc_id(), expected_id);

    Ok(())
}

#[test]
fn missing_target_fails() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("missing.vdraft");

    let result = AddTrackRequest::begin(path);

    assert!(matches!(result, Err(VdiscError::Io(_))));

    Ok(())
}

#[test]
fn corrupt_target_fails() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("corrupt.vdraft");

    fs::write(&path, b"definitely not a valid draft")?;

    let result = AddTrackRequest::begin(path);

    assert!(matches!(result, Err(VdiscError::Serialization(_))));

    Ok(())
}

#[test]
fn burned_disc_path_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("night-drive.vdisc");

    fs::write(&path, b"placeholder")?;

    let result = AddTrackRequest::begin(&path);

    assert!(matches!(result, Err(VdiscError::TargetNotDraft { .. })));

    Ok(())
}

#[test]
fn extensionless_target_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("night-drive");

    fs::write(&path, b"placeholder")?;

    let result = AddTrackRequest::begin(&path);

    assert!(matches!(result, Err(VdiscError::TargetNotDraft { .. })));

    Ok(())
}

#[test]
fn beginning_request_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("night-drive.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&path, &draft)?;

    let before = fs::read(&path)?;

    let _request = AddTrackRequest::begin(&path)?;

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn failed_request_does_not_modify_target() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("wrong.vdisc");

    let original = b"do not touch this";

    fs::write(&path, original)?;

    let result = AddTrackRequest::begin(&path);

    assert!(result.is_err());

    let after = fs::read(&path)?;

    assert_eq!(after, original);

    Ok(())
}
