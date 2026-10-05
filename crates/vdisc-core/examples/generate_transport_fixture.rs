//! Explicit two-track test fixture, not the production burner.
#[path = "../tests/support/vdisc_fixture.rs"]
mod fixture;

use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = fixture::manifest();
    manifest["title"] = serde_json::json!("Transport control fixture");
    let mut second = manifest["tracks"][0].clone();
    second["path"] = serde_json::json!("tracks/02.wav");
    manifest["tracks"].as_array_mut().unwrap().push(second);
    let entries = fixture::entries(
        manifest,
        vec![
            ("tracks/01.wav".into(), fixture::wav()),
            ("tracks/02.wav".into(), fixture::wav()),
        ],
    );
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/vdisc/transport-two-track.vdisc");
    fs::write(output, fixture::build(&entries, false, false))?;
    Ok(())
}
