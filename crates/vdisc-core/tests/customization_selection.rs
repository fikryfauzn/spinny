mod common;

use std::{error::Error, fs};

use vdisc_core::{CustomizationSession, DraftDisc, VdiscError, save_draft};

#[test]
fn valid_draft_can_be_selected_for_customization() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    let expected_id = draft.id();

    save_draft(&path, &draft)?;

    let session = CustomizationSession::begin(&path)?;

    assert_eq!(session.target_path(), path);

    assert_eq!(session.target_disc_id(), expected_id);

    Ok(())
}

#[test]
fn customization_selection_does_not_modify_draft() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&path, &draft)?;

    let before = fs::read(&path)?;

    let _session = CustomizationSession::begin(&path)?;

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn missing_draft_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("missing.vdraft");

    let result = CustomizationSession::begin(&path);

    assert!(result.is_err());

    Ok(())
}

#[test]
fn corrupt_draft_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("broken.vdraft");

    fs::write(&path, b"this is not JSON")?;

    let result = CustomizationSession::begin(&path);

    assert!(result.is_err());

    Ok(())
}

#[test]
fn burned_disc_is_rejected_as_customization_target() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdisc");

    fs::write(&path, b"placeholder")?;

    let result = CustomizationSession::begin(&path);

    assert!(matches!(
        result,
        Err(VdiscError::CustomizationTargetNotDraft { .. })
    ));

    Ok(())
}

#[test]
fn extensionless_file_is_rejected_as_customization_target() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc");

    fs::write(&path, b"placeholder")?;

    let result = CustomizationSession::begin(&path);

    assert!(matches!(
        result,
        Err(VdiscError::CustomizationTargetNotDraft { .. })
    ));

    Ok(())
}

#[test]
fn unchanged_customization_target_verifies() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&path, &draft)?;

    let session = CustomizationSession::begin(&path)?;

    session.verify_target()?;

    Ok(())
}

#[test]
fn replaced_draft_at_same_path_is_detected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let path = sandbox.path().join("disc.vdraft");

    let original = DraftDisc::new("Original")?;

    save_draft(&path, &original)?;

    let session = CustomizationSession::begin(&path)?;

    let replacement = DraftDisc::new("Replacement")?;

    save_draft(&path, &replacement)?;

    let before = fs::read(&path)?;

    let result = session.verify_target();

    assert!(matches!(
        result,
        Err(VdiscError::DraftIdentityMismatch { .. })
    ));

    let after = fs::read(&path)?;

    assert_eq!(before, after);

    Ok(())
}
