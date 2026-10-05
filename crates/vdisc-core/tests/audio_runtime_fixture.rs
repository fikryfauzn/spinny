use std::io::Read;
use vdisc_core::BurnedDisc;

#[test]
fn audible_fixture_has_two_distinct_stereo_two_second_tones() {
    let disc = BurnedDisc::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/vdisc/audio-runtime-two-track.vdisc"
    ))
    .unwrap();
    assert_eq!(disc.track_count(), 2);
    let mut samples = Vec::new();
    for index in 0..2 {
        let track = &disc.tracks()[index];
        assert_eq!(track.duration_ms, Some(2000));
        let mut bytes = Vec::new();
        disc.track_payload(index)
            .unwrap()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 44100);
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
        assert_eq!(bytes.len(), 44 + 88200 * 4);
        let pcm: Vec<i16> = bytes[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect();
        assert!(pcm.iter().all(|sample| sample.abs() <= 3277));
        assert!(pcm.iter().any(|sample| sample.abs() > 3000));
        assert!(
            pcm.as_chunks::<2>()
                .0
                .iter()
                .all(|frame| frame[0] == frame[1])
        );
        samples.push(pcm);
    }
    assert_ne!(samples[0], samples[1]);
}
