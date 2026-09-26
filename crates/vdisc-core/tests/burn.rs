#![cfg(target_os = "linux")]
#[path = "support/vdisc_fixture.rs"]
mod fixture;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use vdisc_core::{format::validate_vdisc, *};

fn ready(dir: &Path, count: usize) -> (PathBuf, PathBuf) {
    let source = dir.join("song.wav");
    fs::write(&source, fixture::wav()).unwrap();
    let draft = dir.join("disc.vdraft");
    save_draft(&draft, &DraftDisc::new("Night Drive").unwrap()).unwrap();
    for _ in 0..count {
        import(&draft, &source);
    }
    (draft, source)
}
fn import(draft: &Path, source: &Path) {
    let selected = AddTrackRequest::begin(draft)
        .unwrap()
        .select_source(TrackSourceKind::Local)
        .select_local_file(source)
        .unwrap();
    add_validated_local_track(ValidatedLocalAudio::validate(selected).unwrap()).unwrap();
}
fn no_temps(dir: &Path) {
    assert!(fs::read_dir(dir).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".vdisc-burn-")
    }));
}
// Independent, test-only extraction from this writer's ZIP64 central records.
fn payloads(path: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let b = fs::read(path).unwrap();
    let u16at = |p| u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as usize;
    let u64at = |p| u64::from_le_bytes(b[p..p + 8].try_into().unwrap()) as usize;
    let end = b.len() - 98;
    let count = u64at(end + 32);
    let mut p = u64at(end + 48);
    let mut result = std::collections::BTreeMap::new();
    for _ in 0..count {
        let name_len = u16at(p + 28);
        let extra = u16at(p + 30);
        let name = String::from_utf8(b[p + 46..p + 46 + name_len].to_vec()).unwrap();
        let x = p + 46 + name_len + 4;
        let len = u64at(x);
        let local = u64at(x + 16);
        let data = local + 30 + u16at(local + 26) + u16at(local + 28);
        result.insert(name, b[data..data + len].to_vec());
        p += 46 + name_len + extra;
    }
    result
}
#[test]
fn burns_one_and_six_tracks_with_exact_bytes_and_order() {
    for count in [1, 6] {
        let dir = tempfile::tempdir().unwrap();
        let (draft, source) = ready(dir.path(), count);
        let before = fs::read(&draft).unwrap();
        let bytes = fs::read(&source).unwrap();
        let result = burn(&draft, dir.path().join("disc.vdisc")).unwrap();
        assert_eq!(result.track_count, count);
        assert_eq!(
            result.size_bytes,
            fs::metadata(&result.output_path).unwrap().len()
        );
        assert!(matches!(result.durability, BurnDurability::Synced));
        let checked = validate_vdisc(&result.output_path).unwrap();
        let m = checked.manifest();
        assert_eq!(m.disc_id, result.disc_id.to_string());
        assert_eq!(m.burned_at_unix, result.burned_at_unix);
        assert_eq!(m.appearance.base_color, format::Rgb::default());
        let embedded = payloads(&result.output_path);
        for i in 1..=count {
            assert_eq!(embedded[&format!("tracks/{i:02}.wav")], bytes);
        }
        assert_eq!(fs::read(draft).unwrap(), before);
        assert_eq!(fs::read(source).unwrap(), bytes);
        no_temps(dir.path());
    }
}
#[test]
fn fresh_identity_and_independence_from_originals() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, source) = ready(dir.path(), 1);
    let a = burn(&draft, dir.path().join("one.vdisc")).unwrap();
    let b = burn(&draft, dir.path().join("two.vdisc")).unwrap();
    assert_ne!(a.disc_id, b.disc_id);
    assert_eq!(a.disc_id.get_version_num(), 4);
    assert_eq!(b.disc_id.get_version_num(), 4);
    assert_ne!(a.disc_id, load_draft(&draft).unwrap().id());
    fs::remove_file(draft).unwrap();
    fs::remove_file(source).unwrap();
    validate_vdisc(a.output_path).unwrap();
    validate_vdisc(b.output_path).unwrap();
}
#[test]
fn all_artwork_formats_and_text_are_preserved() {
    for (format, ext) in [
        (image::ImageFormat::Png, "png"),
        (image::ImageFormat::Jpeg, "jpeg"),
        (image::ImageFormat::WebP, "webp"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (draft, _) = ready(dir.path(), 1);
        let art = dir.path().join(format!("art.{ext}"));
        image::RgbImage::from_pixel(8, 8, image::Rgb([10, 20, 30]))
            .save_with_format(&art, format)
            .unwrap();
        let session = CustomizationSession::begin(&draft).unwrap();
        session.set_disc_color(DiscColor::new(1, 2, 3)).unwrap();
        session.set_disc_label("Label").unwrap();
        session.set_disc_subtitle("Subtitle").unwrap();
        session.set_disc_image(&art).unwrap();
        let result = burn(&draft, dir.path().join("art.vdisc")).unwrap();
        let checked = validate_vdisc(&result.output_path).unwrap();
        assert_eq!(
            checked.manifest().appearance.label.as_deref(),
            Some("Label")
        );
        assert_eq!(
            checked.manifest().appearance.subtitle.as_deref(),
            Some("Subtitle")
        );
        assert_eq!(
            checked.manifest().appearance.base_color,
            format::Rgb { r: 1, g: 2, b: 3 }
        );
        assert_eq!(
            payloads(&result.output_path)[&format!("artwork/disc.{ext}")],
            fs::read(art).unwrap()
        );
    }
}
#[test]
fn mixed_codecs_and_metadata_survive() {
    let dir = tempfile::tempdir().unwrap();
    let draft = dir.path().join("disc.vdraft");
    save_draft(&draft, &DraftDisc::new("Mix").unwrap()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio");
    let extensions = ["flac", "mp3", "opus", "wav"];
    for ext in extensions {
        import(&draft, &root.join(format!("valid.{ext}")));
    }
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&draft).unwrap()).unwrap();
    for track in value["tracks"].as_array_mut().unwrap() {
        for name in ["title", "artist", "album", "genre"] {
            track[name] = serde_json::json!("夜の音楽");
        }
        for name in [
            "source_track_number",
            "source_track_total",
            "source_disc_number",
            "source_disc_total",
        ] {
            track[name] = serde_json::json!(2);
        }
    }
    fs::write(&draft, serde_json::to_vec(&value).unwrap()).unwrap();
    let result = burn(&draft, dir.path().join("mix.vdisc")).unwrap();
    let checked = validate_vdisc(&result.output_path).unwrap();
    let embedded = payloads(&result.output_path);
    for (i, t) in checked.manifest().tracks.iter().enumerate() {
        assert_eq!(t.genre.as_deref(), Some("夜の音楽"));
        assert_eq!(t.source_disc_total, Some(2));
        assert_eq!(
            embedded[&t.path],
            fs::read(root.join(format!("valid.{}", extensions[i]))).unwrap()
        );
    }
}
#[test]
fn occupied_destinations_including_dangling_symlink_survive() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path(), 1);
    for kind in 0..3 {
        let out = dir.path().join(format!("{kind}.vdisc"));
        match kind {
            0 => fs::write(&out, b"existing").unwrap(),
            1 => fs::create_dir(&out).unwrap(),
            _ => std::os::unix::fs::symlink("missing-target", &out).unwrap(),
        }
        assert!(run_preflight(&draft, &out).has_issue(PreflightIssueCode::OutputAlreadyExists));
        assert_eq!(burn(&draft, &out).unwrap_err().phase, BurnPhase::Preflight);
        if kind == 0 {
            assert_eq!(fs::read(&out).unwrap(), b"existing");
        }
        if kind == 2 {
            assert_eq!(fs::read_link(&out).unwrap(), Path::new("missing-target"));
        }
        no_temps(dir.path());
    }
}
#[test]
fn invalid_inputs_leave_no_output_or_temporary() {
    for kind in 0..6 {
        let dir = tempfile::tempdir().unwrap();
        let (draft, source) = ready(dir.path(), 1);
        let mut out = dir.path().join("out.vdisc");
        match kind {
            0 => fs::write(&draft, b"broken").unwrap(),
            1 => fs::remove_file(&source).unwrap(),
            2 => fs::write(&source, b"changed").unwrap(),
            3 => out = dir.path().join("wrong.zip"),
            4 => out = dir.path().join("missing/out.vdisc"),
            _ => save_draft(&draft, &DraftDisc::new("Empty").unwrap()).unwrap(),
        }
        let before = fs::read(&draft).unwrap();
        assert!(burn(&draft, &out).is_err());
        assert!(!out.exists());
        assert_eq!(fs::read(&draft).unwrap(), before);
        no_temps(dir.path());
    }
}
#[test]
fn bare_relative_output_uses_child_cwd() {
    const FLAG: &str = "VDISC_BURN_RELATIVE_CHILD";
    if std::env::var_os(FLAG).is_some() {
        let (draft, _) = ready(Path::new("."), 1);
        let result = burn(draft, "out.vdisc").unwrap();
        assert_eq!(
            result.output_path,
            std::env::current_dir().unwrap().join("out.vdisc")
        );
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "bare_relative_output_uses_child_cwd"])
        .env(FLAG, "1")
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    validate_vdisc(dir.path().join("out.vdisc")).unwrap();
}
#[test]
fn non_utf8_parent_path_works() {
    use std::os::unix::ffi::OsStringExt;
    let dir = tempfile::tempdir().unwrap();
    let sub = dir
        .path()
        .join(std::ffi::OsString::from_vec(vec![b'd', 0xff]));
    fs::create_dir(&sub).unwrap();
    // Draft JSON stores source paths as UTF-8. This test concerns the output
    // directory, which is not serialized into the draft or burned manifest.
    let (draft, source) = ready(dir.path(), 1);
    let draft_before = fs::read(&draft).unwrap();
    let source_before = fs::read(&source).unwrap();
    let result = burn(&draft, sub.join("out.vdisc")).unwrap();
    assert_eq!(result.output_path, sub.join("out.vdisc"));
    assert!(result.output_path.as_os_str().to_str().is_none());
    validate_vdisc(&result.output_path).unwrap();
    assert_eq!(
        payloads(&result.output_path)["tracks/01.wav"],
        source_before
    );
    assert_eq!(fs::read(&draft).unwrap(), draft_before);
    assert_eq!(fs::read(&source).unwrap(), source_before);
    no_temps(&sub);
}
