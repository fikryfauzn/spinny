#![cfg(target_os = "linux")]

#[path = "support/vdisc_fixture.rs"]
mod fixture;

use std::{
    error::Error,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    ops::Range,
    path::{Path, PathBuf},
};

use vdisc_core::{
    format::{FormatErrorKind, Rgb},
    *,
};

type TestResult = std::result::Result<(), Box<dyn Error>>;

fn ready(dir: &Path) -> (PathBuf, PathBuf, Vec<u8>) {
    let source = dir.join("song.wav");
    let source_bytes = fixture::wav();
    fs::write(&source, &source_bytes).unwrap();

    let draft = dir.join("disc.vdraft");
    save_draft(&draft, &DraftDisc::new("Night Drive").unwrap()).unwrap();

    let selected = AddTrackRequest::begin(&draft)
        .unwrap()
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source)
        .unwrap();
    add_validated_local_track(ValidatedLocalAudio::validate(selected).unwrap()).unwrap();

    (draft, source, source_bytes)
}

fn burn_synced(draft: &Path, output: &Path) -> BurnResult {
    let result = burn(draft, output).unwrap();
    assert!(matches!(result.durability, BurnDurability::Synced));
    result
}

fn stored_payload_range(path: &Path, wanted: &str) -> Range<usize> {
    let bytes = fs::read(path).unwrap();
    let u16_at =
        |offset| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
    let u64_at =
        |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;

    let zip64_end = bytes.len() - 98;
    let count = u64_at(zip64_end + 32);
    let mut central = u64_at(zip64_end + 48);

    for _ in 0..count {
        let name_len = u16_at(central + 28);
        let extra_len = u16_at(central + 30);
        let name = std::str::from_utf8(&bytes[central + 46..central + 46 + name_len]).unwrap();
        let zip64_values = central + 46 + name_len + 4;
        let size = u64_at(zip64_values);
        let local = u64_at(zip64_values + 16);
        let data = local + 30 + u16_at(local + 26) + u16_at(local + 28);

        if name == wanted {
            return data..data + size;
        }

        central += 46 + name_len + extra_len;
    }

    panic!("payload {wanted} not found");
}

#[test]
fn burner_output_opens_with_ordered_metadata_and_appearance() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, _) = ready(dir.path());
    let artwork = dir.path().join("art.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([10, 20, 30]))
        .save_with_format(&artwork, image::ImageFormat::Png)?;

    let artwork_bytes = fs::read(&artwork)?;
    let session = CustomizationSession::begin(&draft)?;
    session.set_disc_color(DiscColor::new(1, 2, 3))?;
    session.set_disc_label("Label")?;
    session.set_disc_subtitle("Volume One")?;
    session.set_disc_image(&artwork)?;

    let output = dir.path().join("night-drive.vdisc");
    let burned = burn_synced(&draft, &output);
    fs::remove_file(&artwork)?;
    let disc = BurnedDisc::open(&output)?;

    let burned_id = burned.disc_id.to_string();
    assert_eq!(disc.disc_id(), burned_id.as_str());
    assert_eq!(disc.title(), "Night Drive");
    assert_eq!(disc.burned_at_unix(), burned.burned_at_unix);
    assert_eq!(disc.track_count(), 1);
    assert_eq!(disc.tracks()[0].path, "tracks/01.wav");
    assert_eq!(disc.appearance().label.as_deref(), Some("Label"));
    assert_eq!(disc.appearance().subtitle.as_deref(), Some("Volume One"));
    assert_eq!(disc.appearance().base_color, Rgb { r: 1, g: 2, b: 3 });
    assert_eq!(disc.artwork_bytes()?.unwrap(), artwork_bytes);

    Ok(())
}

#[test]
fn boss_test_disc_survives_deleted_draft_and_source() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, source, source_bytes) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    let _ = burn_synced(&draft, &output);

    fs::remove_file(&draft)?;
    fs::remove_file(&source)?;

    let disc = BurnedDisc::open(&output)?;
    assert_eq!(disc.title(), "Night Drive");
    assert_eq!(disc.track_count(), 1);

    let mut payload = disc.track_payload(0)?.expect("track 1 exists");
    let mut embedded = Vec::new();
    payload.read_to_end(&mut embedded)?;
    assert_eq!(embedded, source_bytes);

    Ok(())
}

#[test]
fn ordered_tracks_and_payloads_follow_manifest_order() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, wav_bytes) = ready(dir.path());
    let flac = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio/valid.flac");
    let flac_bytes = fs::read(&flac)?;

    let selected = AddTrackRequest::begin(&draft)?
        .select_source(TrackSourceKind::Local)
        .select_local_file(&flac)?;
    add_validated_local_track(ValidatedLocalAudio::validate(selected)?)?;

    let output = dir.path().join("ordered.vdisc");
    let _ = burn_synced(&draft, &output);
    let disc = BurnedDisc::open(&output)?;

    assert_eq!(disc.track_count(), 2);
    assert_eq!(disc.tracks()[0].path, "tracks/01.wav");
    assert_eq!(disc.tracks()[1].path, "tracks/02.flac");

    let mut first_payload = disc.track_payload(0)?.expect("first track");
    let mut first = Vec::new();
    first_payload.read_to_end(&mut first)?;
    let mut second_payload = disc.track_payload(1)?.expect("second track");
    let mut second = Vec::new();
    second_payload.read_to_end(&mut second)?;

    assert_eq!(first, wav_bytes);
    assert_eq!(second, flac_bytes);

    Ok(())
}

#[test]
fn track_payload_is_bounded_and_seekable() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, source_bytes) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    let _ = burn_synced(&draft, &output);
    let disc = BurnedDisc::open(&output)?;

    let mut payload = disc.track_payload(0)?.expect("track 1 exists");
    assert_eq!(payload.len(), source_bytes.len() as u64);
    assert!(!payload.is_empty());
    assert_eq!(payload.position(), 0);

    let end = payload.seek(SeekFrom::End(0))?;
    assert_eq!(end, payload.len());
    assert!(payload.seek(SeekFrom::Current(1)).is_err());
    assert!(payload.seek(SeekFrom::Start(payload.len() + 1)).is_err());

    payload.seek(SeekFrom::End(-3))?;
    let mut tail = [0; 16];
    let read = payload.read(&mut tail)?;
    assert_eq!(read, 3);
    assert_eq!(&tail[..read], &source_bytes[source_bytes.len() - 3..]);
    assert_eq!(payload.read(&mut tail)?, 0);
    assert!(disc.track_payload(99)?.is_none());

    Ok(())
}

#[test]
fn pathname_replacement_does_not_switch_the_open_reader() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, source_bytes) = ready(dir.path());
    let output = dir.path().join("disc.vdisc");
    let moved = dir.path().join("moved.vdisc");
    let _ = burn_synced(&draft, &output);

    let disc = BurnedDisc::open(&output)?;
    fs::rename(&output, &moved)?;
    fs::write(&output, b"replacement path contents")?;

    let mut payload = disc.track_payload(0)?.expect("track 1 exists");
    let mut embedded = Vec::new();
    payload.read_to_end(&mut embedded)?;

    assert_eq!(embedded, source_bytes);
    disc.verify_open_file()?;
    assert!(disc.verify_current_path().is_err());

    Ok(())
}

#[test]
fn in_place_mutation_is_detected_on_open_handle_reverification() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, _) = ready(dir.path());
    let output = dir.path().join("disc.vdisc");
    let _ = burn_synced(&draft, &output);

    let disc = BurnedDisc::open(&output)?;
    let range = stored_payload_range(&output, "tracks/01.wav");
    let mut file = fs::OpenOptions::new().write(true).open(&output)?;
    file.seek(SeekFrom::Start(range.start as u64))?;
    file.write_all(&[0xff])?;
    file.sync_all()?;

    let error = disc.verify_open_file().unwrap_err();
    assert_eq!(error.kind, FormatErrorKind::IntegrityMismatch);

    Ok(())
}

#[test]
fn valid_path_replacement_is_detected_as_a_different_disc() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (draft, _, _) = ready(dir.path());
    let output = dir.path().join("disc.vdisc");
    let original = dir.path().join("original.vdisc");
    let _ = burn_synced(&draft, &output);
    let disc = BurnedDisc::open(&output)?;

    fs::rename(&output, &original)?;
    CustomizationSession::begin(&draft)?.set_disc_label("Second pressing")?;
    let _ = burn_synced(&draft, &output);

    let error = disc.verify_current_path().unwrap_err();
    assert_eq!(error.kind, FormatErrorKind::IntegrityMismatch);
    disc.verify_open_file()?;

    Ok(())
}

#[test]
fn malformed_and_missing_payload_archives_are_rejected() -> TestResult {
    let dir = tempfile::tempdir()?;

    let corrupt = dir.path().join("corrupt.vdisc");
    let mut entries = fixture::minimal_entries();
    let payload = entries[1].1.len();
    entries[1].1[payload - 1] ^= 1;
    fs::write(&corrupt, fixture::build(&entries, false, false))?;
    assert_eq!(
        BurnedDisc::open(&corrupt).unwrap_err().kind,
        FormatErrorKind::IntegrityMismatch
    );

    let missing = dir.path().join("missing.vdisc");
    let mut entries = fixture::minimal_entries();
    entries.remove(1);
    fs::write(&missing, fixture::build(&entries, false, false))?;
    assert!(BurnedDisc::open(&missing).is_err());

    Ok(())
}

#[test]
fn stored_valid_and_zip64_fixtures_are_readable_through_supported_api() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc");

    for name in ["valid-v1.vdisc", "valid-v1-zip64.vdisc"] {
        let disc = BurnedDisc::open(root.join(name))?;
        let mut payload = disc.track_payload(0)?.expect("fixture track exists");
        let mut bytes = Vec::new();
        payload.read_to_end(&mut bytes)?;
        assert!(!bytes.is_empty());
        disc.verify_open_file()?;
    }

    Ok(())
}
