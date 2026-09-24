mod common;

use std::{error::Error, fs};

use vdisc_core::{DISC_TRACK_CAPACITY, DraftDisc, VdiscError, load_draft, save_draft};

#[test]
fn creates_valid_draft() -> Result<(), Box<dyn Error>> {
    let draft = DraftDisc::new("Night Drive")?;

    assert_eq!(draft.title(), "Night Drive");
    assert_eq!(draft.capacity(), DISC_TRACK_CAPACITY);
    assert_eq!(draft.capacity(), 6);

    assert!(!draft.id().is_nil());
    assert!(draft.created_at_unix() > 0);

    Ok(())
}

#[test]
fn generates_unique_ids() -> Result<(), Box<dyn Error>> {
    let first = DraftDisc::new("First")?;
    let second = DraftDisc::new("Second")?;

    assert_ne!(first.id(), second.id());

    Ok(())
}

#[test]
fn supports_unicode_title() -> Result<(), Box<dyn Error>> {
    let draft = DraftDisc::new("夜のドライブ — Malam")?;

    assert_eq!(draft.title(), "夜のドライブ — Malam");

    Ok(())
}

#[test]
fn rejects_empty_title() {
    let result = DraftDisc::new("");

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));
}

#[test]
fn rejects_whitespace_only_title() {
    let result = DraftDisc::new("   \t\n");

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));
}

#[test]
fn persists_and_reopens_identically() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("night-drive.vdraft");

    let original = DraftDisc::new("Night Drive")?;

    save_draft(&path, &original)?;

    drop(original);

    let reopened = load_draft(&path)?;

    assert_eq!(reopened.title(), "Night Drive");
    assert!(!reopened.id().is_nil());
    assert_eq!(reopened.capacity(), 6);

    Ok(())
}

#[test]
fn persistence_roundtrip_preserves_every_field() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("roundtrip.vdraft");

    let original = DraftDisc::new("Roundtrip")?;

    save_draft(&path, &original)?;

    let reopened = load_draft(&path)?;

    assert_eq!(original, reopened);

    Ok(())
}

#[test]
fn corrupt_draft_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("corrupt.vdraft");

    fs::write(&path, b"this is absolutely not json")?;

    let result = load_draft(&path);

    assert!(matches!(result, Err(VdiscError::Serialization(_))));

    Ok(())
}

#[test]
fn unsupported_draft_version_is_rejected() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("future.vdraft");

    let data = r#"
    {
        "draft_version": 999,
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "title": "Future Disc",
        "created_at_unix": 1
    }
    "#;

    fs::write(&path, data)?;

    let result = load_draft(&path);

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));

    Ok(())
}
