#[path = "support/vdisc_fixture.rs"]
mod fixture;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};
use vdisc_core::format::*;

fn validate(bytes: &[u8]) -> FormatResult<ValidatedFormat> {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), bytes).unwrap();
    validate_vdisc(file.path())
}
fn packaged(v: Value, payloads: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    fixture::build(&fixture::entries(v, payloads), false, false)
}
fn cd(bytes: &[u8]) -> Vec<usize> {
    fixture::locations(bytes, 0x02014b50u32.to_le_bytes())
}
fn local(bytes: &[u8]) -> Vec<usize> {
    fixture::locations(bytes, 0x04034b50u32.to_le_bytes())
}
fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn classic_zip64_and_both_descriptor_widths_are_accepted() {
    for wide in [false, true] {
        for descriptor in [false, true] {
            let result = validate(&fixture::build(
                &fixture::minimal_entries(),
                wide,
                descriptor,
            ))
            .unwrap();
            assert_eq!(result.manifest().tracks.len(), 1);
        }
    }
}
#[test]
fn six_tracks_and_intentional_duplicate_songs_are_accepted() {
    let mut v = fixture::manifest();
    let mut tracks = Vec::new();
    let mut payloads = Vec::new();
    for i in 1..=6 {
        let path = format!("tracks/{i:02}.wav");
        let mut t = v["tracks"][0].clone();
        t["path"] = json!(path);
        tracks.push(t);
        payloads.push((path, fixture::wav()));
    }
    v["tracks"] = json!(tracks);
    assert_eq!(
        validate(&packaged(v, payloads))
            .unwrap()
            .manifest()
            .tracks
            .len(),
        6
    );
}
#[test]
fn mixed_required_audio_formats_are_accepted() {
    let mut v = fixture::manifest();
    let mut tracks = Vec::new();
    let mut payloads = Vec::new();
    for (i, (ext, container, codec)) in [
        ("flac", "flac", "flac"),
        ("mp3", "mp3", "mp3"),
        ("opus", "ogg", "opus"),
        ("wav", "wav", "pcm"),
    ]
    .into_iter()
    .enumerate()
    {
        let path = format!("tracks/{:02}.{ext}", i + 1);
        tracks.push(json!({"path":path,"container":container,"codec":codec}));
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/audio/valid.{ext}"));
        payloads.push((path, fs::read(source).unwrap()));
    }
    v["tracks"] = json!(tracks);
    validate(&packaged(v, payloads)).unwrap();
}
#[test]
fn physical_entry_order_does_not_control_playback() {
    let mut entries = fixture::minimal_entries();
    entries.reverse();
    let d = validate(&fixture::build(&entries, false, false)).unwrap();
    assert_eq!(d.manifest().tracks[0].path, "tracks/01.wav");
}
#[test]
fn missing_required_entries_are_rejected() {
    for index in 0..3 {
        let mut e = fixture::minimal_entries();
        e.remove(index);
        assert!(validate(&fixture::build(&e, false, false)).is_err());
    }
}
#[test]
fn duplicate_manifest_payload_and_integrity_are_rejected() {
    for index in 0..3 {
        let mut e = fixture::minimal_entries();
        e.push(e[index].clone());
        assert_eq!(
            validate(&fixture::build(&e, false, false))
                .unwrap_err()
                .kind,
            FormatErrorKind::DuplicateEntry
        );
    }
}
#[test]
fn unreferenced_file_is_rejected() {
    let mut e = fixture::minimal_entries();
    e.push(("tracks/02.wav".into(), fixture::wav()));
    assert_eq!(
        validate(&fixture::build(&e, false, false))
            .unwrap_err()
            .kind,
        FormatErrorKind::UnexpectedEntry
    );
}
#[test]
fn corrupt_audio_and_manifest_hashes_are_rejected() {
    for index in [0, 1] {
        let mut e = fixture::minimal_entries();
        if index == 0 {
            e[0].1 = String::from_utf8(e[0].1.clone())
                .unwrap()
                .replace("Night Drive", "Wrong Title")
                .into_bytes();
        } else {
            let n = e[1].1.len();
            e[1].1[n - 1] ^= 1;
        }
        assert_eq!(
            validate(&fixture::build(&e, false, false))
                .unwrap_err()
                .kind,
            FormatErrorKind::IntegrityMismatch
        );
    }
}
#[test]
fn malformed_manifest_and_future_versions_are_rejected() {
    let mut e = fixture::minimal_entries();
    e[0].1 = b"{broken".to_vec();
    assert!(validate(&fixture::build(&e, false, false)).is_err());
    let mut v = fixture::manifest();
    v["format_version"] = json!(99);
    assert_eq!(
        validate(&packaged(v, vec![("tracks/01.wav".into(), fixture::wav())]))
            .unwrap_err()
            .kind,
        FormatErrorKind::UnsupportedVersion
    );
}
#[test]
fn traversal_and_alternate_names_are_rejected() {
    for name in [
        "../track.wav",
        "/tracks/01.wav",
        "tracks\\01.wav",
        "TRACKS/01.wav",
        "tracks//01.wav",
        "tracks/",
    ] {
        let mut e = fixture::minimal_entries();
        e[1].0 = name.into();
        assert!(
            validate(&fixture::build(&e, false, false)).is_err(),
            "{name}"
        );
    }
}
#[test]
fn local_header_disagreement_is_rejected() {
    let original = fixture::build(&fixture::minimal_entries(), false, false);
    for field in [6, 8, 14, 18, 22, 30] {
        let mut b = original.clone();
        b[field] ^= 1;
        assert!(validate(&b).is_err(), "{field}");
    }
}
#[test]
fn unsupported_zip_features_are_rejected() {
    let original = fixture::build(&fixture::minimal_entries(), false, false);
    let first_cd = cd(&original)[0];
    for field in [8, 10, 32, 34] {
        let mut b = original.clone();
        put16(&mut b, first_cd + field, 1);
        assert!(validate(&b).is_err());
    }
    for attrs in [0xa000u32 << 16, 0x4000u32 << 16, 0x10, 0x08] {
        let mut b = original.clone();
        put32(&mut b, first_cd + 38, attrs);
        assert!(validate(&b).is_err());
    }
}
#[test]
fn overlapping_offsets_and_out_of_bounds_sizes_are_rejected() {
    let original = fixture::build(&fixture::minimal_entries(), false, false);
    let centers = cd(&original);
    for offset in [0, u32::MAX - 1] {
        let mut b = original.clone();
        put32(&mut b, centers[1] + 42, offset);
        assert!(validate(&b).is_err());
    }
    let mut b = original.clone();
    put32(&mut b, centers[0] + 20, 0xffff0000);
    put32(&mut b, centers[0] + 24, 0xffff0000);
    assert!(validate(&b).is_err());
}
#[test]
fn bad_crc_and_descriptor_are_rejected() {
    let mut b = fixture::build(&fixture::minimal_entries(), false, false);
    let pos = local(&b)[1];
    let name_len = u16::from_le_bytes([b[pos + 26], b[pos + 27]]) as usize;
    b[pos + 30 + name_len + 50] ^= 1;
    assert_eq!(
        validate(&b).unwrap_err().kind,
        FormatErrorKind::IntegrityMismatch
    );
    let mut b = fixture::build(&fixture::minimal_entries(), true, true);
    let descriptor = fixture::locations(&b, 0x08074b50u32.to_le_bytes())[0];
    b[descriptor + 4] ^= 1;
    assert!(validate(&b).is_err());
}
#[test]
fn truncated_archives_prefixes_trailers_and_comments_are_rejected() {
    let b = fixture::build(&fixture::minimal_entries(), false, false);
    for n in [0, 1, 21, 30, b.len() / 2, b.len() - 1] {
        assert!(validate(&b[..n]).is_err());
    }
    let mut prefix = vec![0];
    prefix.extend(&b);
    assert!(validate(&prefix).is_err());
    let mut trailer = b.clone();
    trailer.push(0);
    assert!(validate(&trailer).is_err());
    let mut comment = b.clone();
    let n = comment.len();
    put16(&mut comment, n - 2, 1);
    comment.push(b'x');
    assert!(validate(&comment).is_err());
}
#[test]
fn split_and_malformed_zip64_are_rejected() {
    let mut b = fixture::build(&fixture::minimal_entries(), false, false);
    let n = b.len();
    put16(&mut b, n - 22 + 4, 1);
    assert!(validate(&b).is_err());
    let original = fixture::build(&fixture::minimal_entries(), true, false);
    let end = fixture::locations(&original, 0x06064b50u32.to_le_bytes())[0];
    let loc = fixture::locations(&original, 0x07064b50u32.to_le_bytes())[0];
    for p in [end + 4, end + 16, loc + 4, loc + 8, loc + 16] {
        let mut b = original.clone();
        b[p] ^= 1;
        assert!(validate(&b).is_err(), "{p}");
    }
}
#[test]
fn entry_count_is_bounded_before_reading_directory() {
    let mut b = fixture::build(&fixture::minimal_entries(), false, false);
    let n = b.len();
    put16(&mut b, n - 22 + 8, 10);
    put16(&mut b, n - 22 + 10, 10);
    assert_eq!(
        validate(&b).unwrap_err().kind,
        FormatErrorKind::ResourceLimit
    );
}
#[test]
fn integrity_inventory_size_and_nulls_are_rejected() {
    for change in 0..4 {
        let mut e = fixture::minimal_entries();
        let mut i: Value = serde_json::from_slice(&e[2].1).unwrap();
        match change {
            0 => {
                i["entries"].as_array_mut().unwrap().remove(1);
            }
            1 => i["entries"][1]["path"] = json!("tracks/02.wav"),
            2 => i["entries"][1]["size_bytes"] = json!(999),
            _ => i["entries"][0]["sha256"] = Value::Null,
        };
        e[2].1 = serde_json::to_vec(&i).unwrap();
        assert!(validate(&fixture::build(&e, false, false)).is_err());
    }
}
#[test]
fn malformed_and_misdeclared_audio_are_rejected_even_with_valid_hashes() {
    let v = fixture::manifest();
    assert_eq!(
        validate(&packaged(
            v,
            vec![("tracks/01.wav".into(), b"not audio".to_vec())]
        ))
        .unwrap_err()
        .kind,
        FormatErrorKind::UnsupportedMedia
    );
    let mut v = fixture::manifest();
    v["tracks"][0] = json!({"path":"tracks/01.flac","container":"flac","codec":"flac"});
    assert!(
        validate(&packaged(
            v,
            vec![("tracks/01.flac".into(), fixture::wav())]
        ))
        .is_err()
    );
}
#[test]
fn all_supported_artwork_formats_are_verified() {
    for (format, name) in [
        (image::ImageFormat::Png, "png"),
        (image::ImageFormat::Jpeg, "jpeg"),
        (image::ImageFormat::WebP, "webp"),
    ] {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(12, 8, image::Rgb([40, 80, 160]))
            .write_to(&mut bytes, format)
            .unwrap();
        let path = format!("artwork/disc.{name}");
        let mut v = fixture::manifest();
        v["appearance"]["image"] = json!({"path":path,"format":name,"width":12,"height":8});
        validate(&packaged(
            v,
            vec![
                ("tracks/01.wav".into(), fixture::wav()),
                (path, bytes.into_inner()),
            ],
        ))
        .unwrap();
    }
}
#[test]
fn bad_artwork_missing_artwork_and_wrong_dimensions_are_rejected() {
    let mut bytes = Cursor::new(Vec::new());
    image::RgbImage::new(12, 8)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let mut v = fixture::manifest();
    v["appearance"]["image"] =
        json!({"path":"artwork/disc.png","format":"png","width":11,"height":8});
    assert!(
        validate(&packaged(
            v.clone(),
            vec![
                ("tracks/01.wav".into(), fixture::wav()),
                ("artwork/disc.png".into(), bytes.into_inner())
            ]
        ))
        .is_err()
    );
    assert!(
        validate(&packaged(
            v.clone(),
            vec![("tracks/01.wav".into(), fixture::wav())]
        ))
        .is_err()
    );
    assert!(
        validate(&packaged(
            v,
            vec![
                ("tracks/01.wav".into(), fixture::wav()),
                ("artwork/disc.png".into(), b"fake".to_vec())
            ]
        ))
        .is_err()
    );
}
#[test]
fn source_independence_and_read_only_verification() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let b = fixture::build(&fixture::minimal_entries(), false, false);
    file.write_all(&b).unwrap();
    let copy = file.path().with_extension("copy.vdisc");
    fs::copy(file.path(), &copy).unwrap();
    let id = validate_vdisc(file.path())
        .unwrap()
        .manifest()
        .disc_id
        .clone();
    drop(file);
    assert_eq!(validate_vdisc(&copy).unwrap().manifest().disc_id, id);
    assert_eq!(fs::read(&copy).unwrap(), b);
    fs::remove_file(copy).unwrap();
}
#[test]
fn stored_repository_fixtures_are_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc");
    for name in ["valid-v1.vdisc", "valid-v1-zip64.vdisc"] {
        validate_vdisc(root.join(name)).unwrap();
    }
    for name in [
        "corrupt-payload.vdisc",
        "missing-manifest.vdisc",
        "future-version.vdisc",
        "duplicate-manifest.vdisc",
        "path-traversal.vdisc",
        "malformed-manifest.vdisc",
    ] {
        assert!(validate_vdisc(root.join(name)).is_err(), "{name}");
    }
}
