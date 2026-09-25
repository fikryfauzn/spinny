#[path = "support/vdisc_fixture.rs"]
mod fixture;
use serde_json::{Value, json};
use vdisc_core::format::*;
fn parse(v: &Value) -> FormatResult<Manifest> {
    parse_manifest(&serde_json::to_vec(v).unwrap())
}

#[test]
fn valid_manifest_and_unicode_tags_roundtrip() {
    let mut v = fixture::manifest();
    v["title"] = json!("夜のドライブ");
    for key in ["title", "artist", "album", "genre"] {
        v["tracks"][0][key] = json!("Music");
    }
    for key in [
        "source_track_number",
        "source_track_total",
        "source_disc_number",
        "source_disc_total",
    ] {
        v["tracks"][0][key] = json!(9);
    }
    v["appearance"]["label"] = json!("Label");
    v["appearance"]["subtitle"] = json!("Subtitle");
    let m = parse(&v).unwrap();
    assert_eq!(parse_manifest(&serde_json::to_vec(&m).unwrap()).unwrap(), m);
}
#[test]
fn unknown_version_precedes_v1_schema_binding() {
    let v = json!({"format_version":2,"future":[null,1.25]});
    assert_eq!(
        parse(&v).unwrap_err().kind,
        FormatErrorKind::UnsupportedVersion
    );
}
#[test]
fn missing_optional_metadata_is_valid() {
    let mut v = fixture::manifest();
    let t = v["tracks"][0].as_object_mut().unwrap();
    for key in ["duration_ms", "sample_rate_hz", "channels"] {
        t.remove(key);
    }
    let m = parse(&v).unwrap();
    assert_eq!(m.tracks[0].duration_ms, None);
}
#[test]
fn zero_and_seven_tracks_are_rejected() {
    for n in [0, 7] {
        let mut v = fixture::manifest();
        v["tracks"] = json!(vec![v["tracks"][0].clone(); n]);
        assert!(parse(&v).is_err());
    }
}
#[test]
fn null_unknown_and_missing_required_fields_are_rejected() {
    for pointer in [
        "/disc_id",
        "/title",
        "/burned_at_unix",
        "/tracks",
        "/appearance",
        "/appearance/base_color",
        "/tracks/0/path",
    ] {
        let mut v = fixture::manifest();
        *v.pointer_mut(pointer).unwrap() = Value::Null;
        assert!(parse(&v).is_err(), "{pointer}");
    }
    for field in ["disc_id", "title", "burned_at_unix", "tracks", "appearance"] {
        let mut v = fixture::manifest();
        v.as_object_mut().unwrap().remove(field);
        assert!(parse(&v).is_err(), "{field}");
    }
    for pointer in ["", "/appearance", "/appearance/base_color", "/tracks/0"] {
        let mut v = fixture::manifest();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(1));
        assert!(parse(&v).is_err(), "{pointer}");
    }
}
#[test]
fn duplicate_keys_at_all_depths_are_rejected() {
    let base = String::from_utf8(serde_json::to_vec(&fixture::manifest()).unwrap()).unwrap();
    for (from, to) in [
        (
            "\"format_version\":1",
            "\"format_version\":1,\"format_version\":1",
        ),
        ("\"r\":255", "\"r\":255,\"r\":255"),
        ("\"codec\":\"pcm\"", "\"codec\":\"pcm\",\"codec\":\"pcm\""),
    ] {
        assert!(parse_manifest(base.replace(from, to).as_bytes()).is_err());
    }
}
#[test]
fn invalid_numbers_are_rejected() {
    let base = String::from_utf8(serde_json::to_vec(&fixture::manifest()).unwrap()).unwrap();
    for token in ["-1", "-0", "1.0", "1e3", "18446744073709551616", "true"] {
        let b = base.replace("1790301600", token);
        assert!(parse_manifest(b.as_bytes()).is_err(), "{token}");
    }
    for pointer in ["/tracks/0/sample_rate_hz", "/tracks/0/channels"] {
        let mut v = fixture::manifest();
        *v.pointer_mut(pointer).unwrap() = json!(0);
        assert!(parse(&v).is_err());
    }
}
#[test]
fn invalid_uuid_path_order_and_codec_are_rejected() {
    for id in [
        "bad",
        "550e8400-e29b-11d4-a716-446655440000",
        "C08971AC-21EF-4F8F-A164-E10B5BADC909",
    ] {
        let mut v = fixture::manifest();
        v["disc_id"] = json!(id);
        assert!(parse(&v).is_err());
    }
    let mut v = fixture::manifest();
    v["tracks"][0]["path"] = json!("tracks/02.wav");
    assert!(parse(&v).is_err());
    let mut v = fixture::manifest();
    v["tracks"][0]["codec"] = json!("aac");
    assert!(parse(&v).is_err());
}
#[test]
fn unsafe_and_noncanonical_paths_are_rejected() {
    for path in [
        "/tracks/01.wav",
        "../tracks/01.wav",
        "tracks/../01.wav",
        "tracks\\01.wav",
        "C:/01.wav",
        "tracks//01.wav",
        "./tracks/01.wav",
        "Tracks/01.wav",
        "tracks/01.wav\0",
        "tracks/",
        "extra.json",
    ] {
        assert!(check_path(path).is_err(), "{path}");
    }
}
#[test]
fn metadata_limit_is_inclusive() {
    let bytes = serde_json::to_vec(&fixture::manifest()).unwrap();
    let mut at = bytes.clone();
    at.resize(MAX_MANIFEST_BYTES as usize, b' ');
    assert!(parse_manifest(&at).is_ok());
    at.push(b' ');
    assert_eq!(
        parse_manifest(&at).unwrap_err().kind,
        FormatErrorKind::ResourceLimit
    );
}
#[test]
fn image_dimension_limits_are_inclusive() {
    assert!(check_dimensions(4000, 4000).is_ok());
    assert!(check_dimensions(8192, 1).is_ok());
    for (w, h) in [
        (0, 1),
        (1, 0),
        (8193, 1),
        (1, 8193),
        (4001, 4000),
        (u32::MAX, u32::MAX),
    ] {
        assert!(check_dimensions(w, h).is_err());
    }
}
#[test]
fn integrity_requires_unique_complete_records_and_lowercase_hashes() {
    let e = fixture::minimal_entries();
    let bytes = &e.last().unwrap().1;
    assert!(parse_integrity(bytes).is_ok());
    let v: Value = serde_json::from_slice(bytes).unwrap();
    for change in 0..5 {
        let mut b = v.clone();
        match change {
            0 => b["algorithm"] = json!("md5"),
            1 => b["entries"][0]["sha256"] = json!("A".repeat(64)),
            2 => b["entries"][0]["size_bytes"] = Value::Null,
            3 => b["entries"][1] = b["entries"][0].clone(),
            _ => b["entries"][0]["path"] = json!("integrity.json"),
        };
        assert!(parse_integrity(&serde_json::to_vec(&b).unwrap()).is_err());
    }
    let mut at = bytes.clone();
    at.resize(MAX_INTEGRITY_BYTES as usize, b' ');
    assert!(parse_integrity(&at).is_ok());
    at.push(b' ');
    assert!(parse_integrity(&at).is_err());
}
#[test]
fn bad_utf8_bom_and_trailing_content_are_rejected() {
    for bytes in [
        vec![0xff],
        b"\xef\xbb\xbf{}".to_vec(),
        [
            serde_json::to_vec(&fixture::manifest()).unwrap(),
            b"x".to_vec(),
        ]
        .concat(),
    ] {
        assert!(parse_manifest(&bytes).is_err());
    }
}
