//! Explicit test-data generator, not a production burner.
#[path = "../tests/support/vdisc_fixture.rs"]
mod fixture;
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc");
    fs::create_dir_all(&root)?;
    let valid = fixture::minimal_entries();
    fs::write(
        root.join("valid-v1.vdisc"),
        fixture::build(&valid, false, false),
    )?;
    fs::write(
        root.join("valid-v1-zip64.vdisc"),
        fixture::build(&valid, true, true),
    )?;
    let mut corrupt = valid.clone();
    let n = corrupt[1].1.len();
    corrupt[1].1[n - 1] ^= 1;
    fs::write(
        root.join("corrupt-payload.vdisc"),
        fixture::build(&corrupt, false, false),
    )?;
    let mut missing = valid.clone();
    missing.remove(0);
    fs::write(
        root.join("missing-manifest.vdisc"),
        fixture::build(&missing, false, false),
    )?;
    let mut future = fixture::manifest();
    future["format_version"] = serde_json::json!(2);
    fs::write(
        root.join("future-version.vdisc"),
        fixture::build(
            &fixture::entries(future, vec![("tracks/01.wav".into(), fixture::wav())]),
            false,
            false,
        ),
    )?;
    let mut duplicate = valid.clone();
    duplicate.push(valid[0].clone());
    fs::write(
        root.join("duplicate-manifest.vdisc"),
        fixture::build(&duplicate, false, false),
    )?;
    let mut traversal = valid.clone();
    traversal[1].0 = "../track.wav".into();
    fs::write(
        root.join("path-traversal.vdisc"),
        fixture::build(&traversal, false, false),
    )?;
    let mut malformed = valid;
    malformed[0].1 = b"{broken".to_vec();
    fs::write(
        root.join("malformed-manifest.vdisc"),
        fixture::build(&malformed, false, false),
    )?;
    Ok(())
}
