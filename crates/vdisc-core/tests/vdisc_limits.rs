use image::{ImageFormat, RgbImage};
use std::{fs, io::Cursor};
use vdisc_core::format::*;
use vdisc_core::{
    CustomizationSession, DiscColor, DraftDisc, SourceFingerprint, load_draft, save_draft,
};

#[test]
fn artwork_encoded_size_limit_is_enforced_before_decode() {
    let mut cursor = Cursor::new(Vec::new());
    RgbImage::new(1, 1)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    let mut bytes = cursor.into_inner();
    bytes.resize(MAX_ARTWORK_BYTES as usize, 0);
    assert_eq!(inspect_artwork(&bytes).unwrap().width, 1);
    bytes.push(0);
    assert_eq!(
        inspect_artwork(&bytes).unwrap_err().kind,
        FormatErrorKind::ResourceLimit
    );
}
#[test]
fn worker_rejects_excessive_dimensions_without_large_output_allocation() {
    let mut c = Cursor::new(Vec::new());
    RgbImage::new(MAX_IMAGE_SIDE + 1, 1)
        .write_to(&mut c, ImageFormat::Png)
        .unwrap();
    assert_eq!(
        inspect_artwork(&c.into_inner()).unwrap_err().kind,
        FormatErrorKind::ResourceLimit
    );
}
#[test]
fn worker_rejects_garbage() {
    assert!(inspect_artwork(b"not an image").is_err());
}
#[test]
fn oversized_artwork_cannot_mutate_customization() {
    let directory = tempfile::tempdir().unwrap();
    let draft = directory.path().join("disc.vdraft");
    let artwork = directory.path().join("large.png");
    save_draft(&draft, &DraftDisc::new("Disc").unwrap()).unwrap();
    let session = CustomizationSession::begin(&draft).unwrap();
    session.set_disc_color(DiscColor::new(1, 2, 3)).unwrap();
    let before = fs::read(&draft).unwrap();
    fs::File::create(&artwork)
        .unwrap()
        .set_len(MAX_ARTWORK_BYTES + 1)
        .unwrap();
    assert!(session.set_disc_image(&artwork).is_err());
    assert_eq!(fs::read(&draft).unwrap(), before);
}
#[test]
fn in_memory_fingerprint_matches_file_fingerprint() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"abc").unwrap();
    let actual = SourceFingerprint::from_bytes(b"abc");
    assert_eq!(actual, SourceFingerprint::from_file(file.path()).unwrap());
    assert_eq!(
        actual.sha256(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
#[test]
fn draft_projection_preserves_metadata_and_drops_local_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("disc.vdraft");
    let mut v = serde_json::to_value(DraftDisc::new("Disc").unwrap()).unwrap();
    v["tracks"] = serde_json::json!([{
        "id":"21c7e339-b7da-4bed-a2e2-1cd6b6d80130","source_path":"/private/song.mp3",
        "source_fingerprint":{"size_bytes":123,"sha256":"a".repeat(64)},
        "container":"mpa","codec":"mp3","sample_rate":48000,"channels":2,"duration_ms":1234,
        "title":"Song","artist":"Artist","album":"Album","genre":"Rock",
        "source_track_number":9,"source_track_total":10,"source_disc_number":2,"source_disc_total":3
    }]);
    fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    let draft = load_draft(&path).unwrap();
    let id = uuid::Uuid::new_v4();
    let manifest = Manifest::from_draft(&draft, id, 123).unwrap();
    let encoded = serde_json::to_string(&manifest).unwrap();
    assert_eq!(manifest.disc_id, id.to_string());
    assert_eq!(manifest.tracks[0].container, "mp3");
    assert_eq!(manifest.tracks[0].source_disc_total, Some(3));
    assert_eq!(manifest.tracks[0].genre.as_deref(), Some("Rock"));
    assert!(!encoded.contains("/private/"));
    assert!(!encoded.contains("source_fingerprint"));
    assert!(!encoded.contains("21c7e339"));
    assert_ne!(manifest.disc_id, draft.id().to_string());
}
