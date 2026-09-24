mod common;

use std::{error::Error, fs};

use vdisc_core::DISC_TRACK_CAPACITY;

#[test]
fn disc_capacity_contract_is_six() {
    assert_eq!(DISC_TRACK_CAPACITY, 6);
}

#[test]
fn test_sandbox_is_temporary() -> Result<(), Box<dyn Error>> {
    let sandbox = common::TestSandbox::new()?;

    let sandbox_path = sandbox.path().to_path_buf();
    let sentinel = sandbox.path().join("sentinel");

    fs::write(&sentinel, b"temporary test data")?;

    assert!(sentinel.exists());

    drop(sandbox);

    assert!(!sandbox_path.exists());

    Ok(())
}
