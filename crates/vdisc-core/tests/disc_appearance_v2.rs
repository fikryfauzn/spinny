//! Expanded appearance and migration acceptance tests.

mod common;

use std::{error::Error, fs, path::Path};

use image::{ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};
use vdisc_core::{CustomizationSession, DiscColor, DraftDisc, load_draft, save_draft};

type TestResult = Result<(), Box<dyn Error>>;

fn white() -> Value {
    json!({ "r": 255, "g": 255, "b": 255 })
}

fn create_draft(path: &Path) -> TestResult {
    save_draft(path, &DraftDisc::new("Night Drive")?)?;
    Ok(())
}

fn create_png(path: &Path) -> TestResult {
    RgbImage::from_pixel(12, 8, Rgb([40, 80, 160])).save_with_format(path, ImageFormat::Png)?;
    Ok(())
}

fn legacy_draft(surface: Value) -> Value {
    json!({
        "draft_version": 1,
        "id": "c08971ac-21ef-4f8f-a164-e10b5badc909",
        "title": "Night Drive",
        "created_at_unix": 1790301600_u64,
        "tracks": [],
        "appearance": {
            "surface": surface,
            "label": "夜のドライブ"
        }
    })
}

#[test]
fn new_draft_serializes_white_base_in_version_two() -> TestResult {
    let draft = DraftDisc::new("Disc")?;
    let value = serde_json::to_value(&draft)?;

    assert_eq!(value["draft_version"], json!(2));
    assert_eq!(value["appearance"]["base_color"], white());
    assert!(value["appearance"].get("surface").is_none());
    assert!(draft.appearance().image().is_none());
    assert!(draft.appearance().label().is_none());
    Ok(())
}

#[test]
fn selecting_image_preserves_existing_color_after_reload() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;
    let source_before = fs::read(&artwork)?;

    let session = CustomizationSession::begin(&path)?;
    session.set_disc_color(DiscColor::new(18, 52, 86))?;
    session.set_disc_image(&artwork)?;

    let draft = load_draft(&path)?;
    assert!(draft.appearance().image().is_some());
    let value = serde_json::to_value(&draft)?;
    assert_eq!(
        value["appearance"]["base_color"],
        json!({ "r": 18, "g": 52, "b": 86 })
    );
    assert_eq!(fs::read(&artwork)?, source_before);
    Ok(())
}

#[test]
fn selecting_color_preserves_existing_image_after_reload() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;

    let session = CustomizationSession::begin(&path)?;
    let selected_image = session.set_disc_image(&artwork)?;
    session.set_disc_color(DiscColor::new(10, 20, 30))?;

    let draft = load_draft(&path)?;
    assert_eq!(draft.appearance().image(), Some(&selected_image));
    let value = serde_json::to_value(&draft)?;
    assert_eq!(
        value["appearance"]["base_color"],
        json!({ "r": 10, "g": 20, "b": 30 })
    );
    Ok(())
}

#[test]
fn legacy_color_migrates_in_memory_without_rewriting_source() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("legacy.vdraft");
    let original = legacy_draft(json!({
        "kind": "color", "value": { "r": 18, "g": 52, "b": 86 }
    }));
    let original_bytes = serde_json::to_vec_pretty(&original)?;
    fs::write(&path, &original_bytes)?;

    let draft = load_draft(&path)?;
    let migrated = serde_json::to_value(&draft)?;
    assert_eq!(fs::read(&path)?, original_bytes);
    assert_eq!(migrated["draft_version"], json!(2));
    assert_eq!(
        migrated["appearance"]["base_color"],
        json!({ "r": 18, "g": 52, "b": 86 })
    );
    for field in ["id", "title", "created_at_unix", "tracks"] {
        assert_eq!(migrated[field], original[field], "changed {field}");
    }
    assert_eq!(draft.appearance().label(), Some("夜のドライブ"));
    Ok(())
}

#[test]
fn legacy_none_and_missing_appearance_both_migrate_to_white() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("legacy.vdraft");
    let with_none = legacy_draft(json!({ "kind": "none" }));
    let mut without_appearance = with_none.clone();
    without_appearance
        .as_object_mut()
        .expect("fixture is an object")
        .remove("appearance");

    for original in [with_none, without_appearance] {
        let before = serde_json::to_vec_pretty(&original)?;
        fs::write(&path, &before)?;
        let draft = load_draft(&path)?;
        let migrated = serde_json::to_value(&draft)?;
        assert_eq!(migrated["appearance"]["base_color"], white());
        assert!(draft.appearance().image().is_none());
        assert_eq!(fs::read(&path)?, before);
    }
    Ok(())
}

#[test]
fn legacy_image_migration_preserves_image_record_and_supplies_white() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;
    let selected = CustomizationSession::begin(&path)?.set_disc_image(&artwork)?;
    let image_record = serde_json::to_value(&selected)?;
    let original = legacy_draft(json!({ "kind": "image", "value": image_record }));
    let before = serde_json::to_vec_pretty(&original)?;
    fs::write(&path, &before)?;

    let draft = load_draft(&path)?;
    assert_eq!(draft.appearance().image(), Some(&selected));
    let migrated = serde_json::to_value(&draft)?;
    assert_eq!(migrated["appearance"]["base_color"], white());
    assert_eq!(migrated["appearance"]["image"], image_record);
    assert_eq!(fs::read(&path)?, before);
    Ok(())
}

#[test]
fn successful_save_of_migrated_draft_writes_version_two() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("legacy.vdraft");
    let original = legacy_draft(json!({ "kind": "none" }));
    fs::write(&path, serde_json::to_vec_pretty(&original)?)?;

    let draft = load_draft(&path)?;
    save_draft(&path, &draft)?;
    let saved: Value = serde_json::from_slice(&fs::read(&path)?)?;
    assert_eq!(saved["draft_version"], json!(2));
    assert_eq!(saved["id"], original["id"]);
    assert_eq!(saved["created_at_unix"], original["created_at_unix"]);
    assert_eq!(saved["appearance"]["base_color"], white());
    assert!(saved["appearance"].get("surface").is_none());
    assert_eq!(load_draft(&path)?, draft);
    Ok(())
}

#[test]
fn subtitle_roundtrip_preserves_other_appearance_fields() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;
    let session = CustomizationSession::begin(&path)?;
    session.set_disc_color(DiscColor::new(1, 2, 3))?;
    let image = session.set_disc_image(&artwork)?;
    session.set_disc_label("Label")?;
    session.set_disc_subtitle("夜 — Volume One")?;
    let draft = load_draft(&path)?;
    assert_eq!(draft.appearance().subtitle(), Some("夜 — Volume One"));
    assert_eq!(draft.appearance().label(), Some("Label"));
    assert_eq!(draft.appearance().image(), Some(&image));
    assert_eq!(*draft.appearance().base_color(), DiscColor::new(1, 2, 3));
    Ok(())
}

#[test]
fn blank_subtitle_is_rejected_without_mutation() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    create_draft(&path)?;
    let session = CustomizationSession::begin(&path)?;
    session.set_disc_subtitle("Keep me")?;
    let before = fs::read(&path)?;
    for blank in ["", "   ", "\t\n", "\u{2003}"] {
        assert!(session.set_disc_subtitle(blank).is_err());
        assert_eq!(fs::read(&path)?, before);
    }
    Ok(())
}

#[test]
fn clear_operations_are_independent() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;
    let session = CustomizationSession::begin(&path)?;
    session.set_disc_color(DiscColor::new(1, 2, 3))?;
    let image = session.set_disc_image(&artwork)?;
    session.set_disc_label("Label")?;
    session.set_disc_subtitle("Subtitle")?;

    session.clear_disc_color()?;
    let draft = load_draft(&path)?;
    assert_eq!(*draft.appearance().base_color(), DiscColor::WHITE);
    assert_eq!(draft.appearance().image(), Some(&image));
    assert_eq!(draft.appearance().label(), Some("Label"));
    assert_eq!(draft.appearance().subtitle(), Some("Subtitle"));

    session.set_disc_color(DiscColor::new(4, 5, 6))?;
    session.clear_disc_image()?;
    let draft = load_draft(&path)?;
    assert_eq!(*draft.appearance().base_color(), DiscColor::new(4, 5, 6));
    assert!(draft.appearance().image().is_none());
    assert_eq!(draft.appearance().label(), Some("Label"));
    assert_eq!(draft.appearance().subtitle(), Some("Subtitle"));

    session.clear_disc_label()?;
    let draft = load_draft(&path)?;
    assert!(draft.appearance().label().is_none());
    assert_eq!(draft.appearance().subtitle(), Some("Subtitle"));
    assert_eq!(*draft.appearance().base_color(), DiscColor::new(4, 5, 6));

    session.set_disc_label("Keep label")?;
    session.clear_disc_subtitle()?;
    let draft = load_draft(&path)?;
    assert!(draft.appearance().subtitle().is_none());
    assert_eq!(draft.appearance().label(), Some("Keep label"));
    assert_eq!(*draft.appearance().base_color(), DiscColor::new(4, 5, 6));
    Ok(())
}

#[test]
fn clear_surface_resets_visuals_but_preserves_text() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    let artwork = sandbox.path().join("art.png");
    create_draft(&path)?;
    create_png(&artwork)?;
    let session = CustomizationSession::begin(&path)?;
    session.set_disc_color(DiscColor::new(1, 2, 3))?;
    session.set_disc_image(&artwork)?;
    session.set_disc_label("Label")?;
    session.set_disc_subtitle("Subtitle")?;
    session.clear_disc_surface()?;
    let draft = load_draft(&path)?;
    assert_eq!(*draft.appearance().base_color(), DiscColor::WHITE);
    assert!(draft.appearance().image().is_none());
    assert_eq!(draft.appearance().label(), Some("Label"));
    assert_eq!(draft.appearance().subtitle(), Some("Subtitle"));
    Ok(())
}

#[test]
fn version_two_missing_base_color_or_appearance_is_rejected() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("broken.vdraft");
    let valid = serde_json::to_value(DraftDisc::new("Disc")?)?;
    for remove_appearance in [false, true] {
        let mut broken = valid.clone();
        if remove_appearance {
            broken.as_object_mut().unwrap().remove("appearance");
        } else {
            broken["appearance"]
                .as_object_mut()
                .unwrap()
                .remove("base_color");
        }
        let before = serde_json::to_vec_pretty(&broken)?;
        fs::write(&path, &before)?;
        assert!(load_draft(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
    }
    Ok(())
}

#[test]
fn malformed_legacy_appearance_is_rejected_without_rewriting() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("broken.vdraft");
    for surface in [
        json!({ "kind": "color", "value": { "r": 256, "g": 0, "b": 0 } }),
        json!({ "kind": "image", "value": {} }),
        json!({ "kind": "unknown" }),
    ] {
        let before = serde_json::to_vec_pretty(&legacy_draft(surface))?;
        fs::write(&path, &before)?;
        assert!(load_draft(&path).is_err());
        assert_eq!(fs::read(&path)?, before);
    }
    Ok(())
}

#[test]
fn migration_preserves_every_track_field_and_order() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("legacy.vdraft");
    let first = json!({
        "id": "21c7e339-b7da-4bed-a2e2-1cd6b6d80130",
        "source_path": "/missing/song.flac",
        "source_fingerprint": { "size_bytes": 123, "sha256": "a".repeat(64) },
        "container": "flac", "codec": "flac",
        "sample_rate": 48000, "channels": 2, "duration_ms": 1234,
        "title": "Track", "artist": "Artist", "album": "Album", "genre": "Rock",
        "source_track_number": 9, "source_track_total": 10,
        "source_disc_number": 2, "source_disc_total": 3
    });
    let mut second = first.clone();
    second["id"] = json!("ad343c30-54a4-4491-bda9-607b581cf8a3");
    second["title"] = json!("Second");
    let mut original = legacy_draft(json!({ "kind": "none" }));
    original["tracks"] = json!([first, second]);
    fs::write(&path, serde_json::to_vec_pretty(&original)?)?;
    let draft = load_draft(&path)?;
    let migrated = serde_json::to_value(&draft)?;
    assert_eq!(migrated["tracks"], original["tracks"]);
    save_draft(&path, &draft)?;
    assert_eq!(load_draft(&path)?, draft);
    Ok(())
}

#[test]
fn rejected_mutation_does_not_upgrade_legacy_file() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("legacy.vdraft");
    let before = serde_json::to_vec_pretty(&legacy_draft(json!({ "kind": "none" })))?;
    fs::write(&path, &before)?;
    let session = CustomizationSession::begin(&path)?;
    assert!(session.set_disc_subtitle(" ").is_err());
    assert_eq!(fs::read(&path)?, before);
    session.set_disc_subtitle("New subtitle")?;
    let saved: Value = serde_json::from_slice(&fs::read(&path)?)?;
    assert_eq!(saved["draft_version"], json!(2));
    assert_eq!(saved["appearance"]["subtitle"], json!("New subtitle"));
    assert_eq!(saved["appearance"]["label"], json!("夜のドライブ"));
    Ok(())
}

#[test]
fn new_operations_reject_replaced_session_target() -> TestResult {
    let sandbox = common::TestSandbox::new()?;
    let path = sandbox.path().join("disc.vdraft");
    create_draft(&path)?;
    let session = CustomizationSession::begin(&path)?;
    save_draft(&path, &DraftDisc::new("Replacement")?)?;
    let before = fs::read(&path)?;
    let results = [
        session.set_disc_subtitle("Subtitle"),
        session.clear_disc_subtitle(),
        session.clear_disc_color(),
        session.clear_disc_image(),
    ];
    for result in results {
        assert!(matches!(
            result,
            Err(vdisc_core::VdiscError::DraftIdentityMismatch { .. })
        ));
    }
    assert_eq!(fs::read(&path)?, before);
    Ok(())
}
