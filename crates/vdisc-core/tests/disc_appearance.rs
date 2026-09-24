mod common;

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use image::{ImageFormat, Rgb, RgbImage};

use vdisc_core::{
    CustomizationSession, DiscColor, DiscImageFormat, DiscSurface, DraftDisc, VdiscError,
    load_draft, save_draft,
};

fn create_draft(directory: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = directory.join("disc.vdraft");

    let draft = DraftDisc::new("Night Drive")?;

    save_draft(&path, &draft)?;

    Ok(path)
}

fn create_png(path: &Path) -> Result<(), Box<dyn Error>> {
    let image = RgbImage::from_pixel(12, 8, Rgb([40, 80, 160]));

    image.save_with_format(path, ImageFormat::Png)?;

    Ok(())
}

#[test]
fn new_draft_has_no_customization() -> Result<(), Box<dyn Error>> {
    let draft = DraftDisc::new("Disc")?;

    assert!(!draft.appearance().has_surface());

    assert_eq!(draft.appearance().label(), None);

    Ok(())
}

#[test]
fn solid_disc_color_is_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_color(DiscColor::new(18, 52, 86))?;

    let reopened = load_draft(&draft_path)?;

    let color = reopened
        .appearance()
        .color()
        .expect("disc should have a color");

    assert_eq!(color.r(), 18);

    assert_eq!(color.g(), 52);

    assert_eq!(color.b(), 86);

    assert_eq!(color.to_hex(), "#123456");

    Ok(())
}

#[test]
fn disc_image_is_validated_and_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    /*
     * Deliberately don't depend on the extension
     * for format detection.
     */
    let image_path = sandbox.path().join("artwork.bin");

    create_png(&image_path)?;

    let session = CustomizationSession::begin(&draft_path)?;

    let selected = session.set_disc_image(&image_path)?;

    assert_eq!(selected.width(), 12);

    assert_eq!(selected.height(), 8);

    assert_eq!(selected.format(), DiscImageFormat::Png);

    let reopened = load_draft(&draft_path)?;

    let image = reopened
        .appearance()
        .image()
        .expect("disc should have image artwork");

    assert_eq!(image.width(), 12);

    assert_eq!(image.height(), 8);

    assert_eq!(image.format(), DiscImageFormat::Png);

    assert_eq!(image.source_path(), fs::canonicalize(&image_path,)?);

    assert_eq!(image.source_fingerprint().sha256().len(), 64);

    Ok(())
}

#[test]
fn image_replaces_existing_color() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let image_path = sandbox.path().join("art.png");

    create_png(&image_path)?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_color(DiscColor::new(255, 0, 0))?;

    session.set_disc_image(&image_path)?;

    let reopened = load_draft(&draft_path)?;

    assert!(reopened.appearance().color().is_none());

    assert!(reopened.appearance().image().is_some());

    Ok(())
}

#[test]
fn color_replaces_existing_image() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let image_path = sandbox.path().join("art.png");

    create_png(&image_path)?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_image(&image_path)?;

    session.set_disc_color(DiscColor::new(10, 20, 30))?;

    let reopened = load_draft(&draft_path)?;

    assert!(reopened.appearance().image().is_none());

    assert_eq!(
        reopened
            .appearance()
            .color()
            .expect("disc should have a color",)
            .to_hex(),
        "#0A141E"
    );

    Ok(())
}

#[test]
fn unicode_disc_label_is_persisted() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_label("夜のドライブ")?;

    let reopened = load_draft(&draft_path)?;

    assert_eq!(reopened.appearance().label(), Some("夜のドライブ"));

    Ok(())
}

#[test]
fn empty_label_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let session = CustomizationSession::begin(&draft_path)?;

    let before = fs::read(&draft_path)?;

    let result = session.set_disc_label("   ");

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn customization_can_be_cleared() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_color(DiscColor::new(1, 2, 3))?;

    session.set_disc_label("My Disc")?;

    session.clear_disc_surface()?;

    session.clear_disc_label()?;

    let reopened = load_draft(&draft_path)?;

    assert!(matches!(reopened.appearance().surface(), DiscSurface::None));

    assert_eq!(reopened.appearance().label(), None);

    Ok(())
}

#[test]
fn fake_image_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let image_path = sandbox.path().join("fake.png");

    fs::write(&image_path, b"this is not an image")?;

    let session = CustomizationSession::begin(&draft_path)?;

    let before = fs::read(&draft_path)?;

    let result = session.set_disc_image(&image_path);

    assert!(result.is_err());

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn selecting_image_never_modifies_source() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let image_path = sandbox.path().join("art.png");

    create_png(&image_path)?;

    let before = fs::read(&image_path)?;

    let session = CustomizationSession::begin(&draft_path)?;

    session.set_disc_image(&image_path)?;

    let after = fs::read(&image_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn replaced_session_target_cannot_be_customized() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let session = CustomizationSession::begin(&draft_path)?;

    let replacement = DraftDisc::new("Replacement")?;

    save_draft(&draft_path, &replacement)?;

    let before = fs::read(&draft_path)?;

    let result = session.set_disc_color(DiscColor::new(255, 255, 255));

    assert!(matches!(
        result,
        Err(VdiscError::DraftIdentityMismatch { .. })
    ));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn missing_image_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let missing_image = sandbox.path().join("missing.png");

    let session = CustomizationSession::begin(&draft_path)?;

    let before = fs::read(&draft_path)?;

    let result = session.set_disc_image(&missing_image);

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn unsupported_image_format_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let draft_path = create_draft(sandbox.path())?;

    let image_path = sandbox.path().join("unsupported.gif");

    /*
     * A tiny GIF-like file.
     *
     * We only need enough data for image format
     * detection. GIF is deliberately unsupported
     * by the VDISC V0.1 appearance model.
     */
    fs::write(
        &image_path,
        b"GIF89a\x01\x00\x01\x00\x80\x00\x00\
          \x00\x00\x00\xff\xff\xff\x3b",
    )?;

    let session = CustomizationSession::begin(&draft_path)?;

    let before = fs::read(&draft_path)?;

    let result = session.set_disc_image(&image_path);

    assert!(matches!(result, Err(VdiscError::InvalidInput(_))));

    let after = fs::read(&draft_path)?;

    assert_eq!(before, after);

    Ok(())
}

#[test]
fn disc_color_supports_full_rgb_domain() {
    let minimum = DiscColor::new(0, 0, 0);

    assert_eq!(minimum.to_hex(), "#000000");

    let maximum = DiscColor::new(255, 255, 255);

    assert_eq!(maximum.to_hex(), "#FFFFFF");
}
