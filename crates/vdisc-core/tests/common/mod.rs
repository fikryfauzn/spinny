use std::{io, path::Path};

use tempfile::TempDir;

pub struct TestSandbox {
    dir: TempDir,
}

impl TestSandbox {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            dir: tempfile::tempdir()?,
        })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}
