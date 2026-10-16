//! Objective 02: partial-trust playback mounting must never expose unchecked
//! audio beyond track zero. Strict BurnedDisc::open must remain unchanged.
use std::{fs, io::Read, path::PathBuf};
use vdisc_core::BurnedDisc;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc")
        .join(name)
}

fn mutate_payload(source: &str, index: usize) -> tempfile::TempPath {
    let input = fixture(source);
    let strict = BurnedDisc::open(&input).unwrap();
    let mut payload = strict.track_payload(index).unwrap().unwrap();
    let mut contents = Vec::new();
    payload.read_to_end(&mut contents).unwrap();
    assert!(contents.len() > 128, "fixture payload must contain audio");
    let mut archive = fs::read(&input).unwrap();
    // Different tracks can contain identical payload bytes. Resolve the
    // selected track by its unique ZIP local-header filename, not by its
    // audio contents. The matching data range must still equal the payload
    // returned by the strict, fully validated reader.
    let name = strict.tracks()[index].path.as_bytes();
    let mut offsets = Vec::new();
    for (header, signature) in archive.windows(4).enumerate() {
        if signature != b"PK\x03\x04" || header + 30 > archive.len() {
            continue;
        }
        let name_len = u16::from_le_bytes([archive[header + 26], archive[header + 27]]) as usize;
        let extra_len = u16::from_le_bytes([archive[header + 28], archive[header + 29]]) as usize;
        let name_start = header + 30;
        let name_end = name_start + name_len;
        let data_start = name_end + extra_len;
        let Some(data_end) = data_start.checked_add(contents.len()) else {
            continue;
        };
        if archive.get(name_start..name_end) == Some(name)
            && archive.get(data_start..data_end) == Some(contents.as_slice())
        {
            offsets.push(data_start);
        }
    }
    assert_eq!(
        offsets.len(),
        1,
        "selected ZIP entry must have one matching local header"
    );
    archive[offsets[0] + contents.len() / 2] ^= 0x80;
    let tmp = tempfile::Builder::new()
        .suffix(".vdisc")
        .tempfile()
        .unwrap();
    fs::write(tmp.path(), archive).unwrap();
    tmp.into_temp_path()
}

#[test]
fn fast_open_keeps_track_zero_available_and_gates_later_tracks() {
    let input = fixture("transport-two-track.vdisc");
    let fast = BurnedDisc::open_for_playback(&input).unwrap();
    let strict = BurnedDisc::open(&input).unwrap();
    assert_eq!(fast.title(), strict.title());
    assert_eq!(fast.track_count(), 2);
    let mut first = fast.track_payload(0).unwrap().unwrap();
    let mut bytes = [0u8; 16];
    assert_eq!(first.read(&mut bytes).unwrap(), 16);
    // The public payload API is nonblocking. Wait for verification in
    // this TEST only; the GUI must never wait on its event thread.
    let start = std::time::Instant::now();
    while !fast.fully_verified() && start.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(fast.fully_verified(), "fixture must pass full validation");
    let mut second = fast.track_payload(1).unwrap().unwrap();
    assert_eq!(second.read(&mut bytes).unwrap(), 16);
    assert!(fast.fully_verified());
}

#[test]
fn modified_first_track_is_rejected_before_fast_mount_returns() {
    let tampered = mutate_payload("transport-two-track.vdisc", 0);
    assert!(BurnedDisc::open_for_playback(&tampered).is_err());
}

#[test]
fn modified_second_track_is_never_available_for_playback() {
    let tampered = mutate_payload("transport-two-track.vdisc", 1);
    // This mount is allowed because the first track and structural metadata
    // are sound. A later unverified track must never escape the gate.
    let fast = BurnedDisc::open_for_playback(&tampered).unwrap();
    // The full verifier may finish before this assertion on a tiny fixture.
    // In that case even track 0 must reject new reads after the late failure.
    if let Err(error) = fast.track_payload(0) {
        assert!(error.to_string().contains("mismatch"), "{error}");
    }
    // Wait only in the test until the worker has reported its failure.
    let start = std::time::Instant::now();
    let err = loop {
        match fast.playback_track_ready(1) {
            Err(error) => break error,
            Ok(false) if start.elapsed() < std::time::Duration::from_secs(5) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            other => panic!("invalid second track was not rejected: {other:?}"),
        }
    };
    assert!(err.to_string().contains("mismatch"), "{err}");
    assert!(fast.track_payload(1).is_err());
    assert!(!fast.fully_verified());
    assert!(BurnedDisc::open(&tampered).is_err());
}

#[test]
fn malformed_metadata_and_missing_disc_fail_without_a_background_handle() {
    assert!(BurnedDisc::open_for_playback(fixture("malformed-manifest.vdisc")).is_err());
    assert!(BurnedDisc::open_for_playback("/missing-fast-mount-disc.vdisc").is_err());
}
