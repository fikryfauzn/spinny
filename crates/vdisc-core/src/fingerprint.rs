use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFingerprint {
    size_bytes: u64,
    sha256: String,
}

impl SourceFingerprint {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self {
            size_bytes: bytes.len() as u64,
            sha256: encode_hex(&digest),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();

        let metadata = std::fs::metadata(path)?;

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];

        loop {
            let read = reader.read(&mut buffer)?;

            if read == 0 {
                break;
            }

            hasher.update(&buffer[..read]);
        }

        let digest = hasher.finalize();

        Ok(Self {
            size_bytes: metadata.len(),
            sha256: encode_hex(&digest),
        })
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(bytes.len() * 2);

    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);

        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}
