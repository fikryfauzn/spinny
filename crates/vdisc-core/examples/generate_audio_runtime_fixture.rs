//! Deterministic native listening fixture. Refuses to overwrite an existing artifact.
#[path = "../tests/support/vdisc_fixture.rs"]
mod fixture;
use std::{fs::OpenOptions, io::Write, path::Path};

fn tone(frequency: f64) -> Vec<u8> {
    const FRAMES: u32 = 88200;
    let data_len = FRAMES * 4;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_len).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(44100u32.to_le_bytes());
    bytes.extend(176400u32.to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_len.to_le_bytes());
    for frame in 0..FRAMES {
        let sample = ((std::f64::consts::TAU * frequency * f64::from(frame) / 44100.0).sin()
            * 3276.7)
            .round() as i16;
        bytes.extend(sample.to_le_bytes());
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = fixture::manifest();
    manifest["title"] = serde_json::json!("Audio runtime listening tones");
    manifest["tracks"] = serde_json::json!([
        {"path":"tracks/01.wav", "container":"wav", "codec":"pcm", "duration_ms":2000, "sample_rate_hz":44100, "channels":2},
        {"path":"tracks/02.wav", "container":"wav", "codec":"pcm", "duration_ms":2000, "sample_rate_hz":44100, "channels":2}
    ]);
    let entries = fixture::entries(
        manifest,
        vec![
            ("tracks/01.wav".into(), tone(440.0)),
            ("tracks/02.wav".into(), tone(660.0)),
        ],
    );
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc/audio-runtime-two-track.vdisc");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&fixture::build(&entries, false, false))?;
    Ok(())
}
