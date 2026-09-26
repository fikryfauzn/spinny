//! Private Stored ZIP64 emitter. Never buffers complete audio payloads.
use super::{
    FormatError, FormatErrorKind as K, FormatResult, IntegrityEntry, MAX_ENTRIES, check_path,
    zip::crc_update,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
};

#[derive(Debug)]
pub(crate) struct EntryReceipt {
    pub record: IntegrityEntry,
}
struct Record {
    name: String,
    crc: u32,
    size: u64,
    offset: u64,
}
pub(crate) struct StoredZipWriter<W: Write> {
    output: W,
    offset: u64,
    records: Vec<Record>,
    names: BTreeSet<String>,
    failed: bool,
}
fn invalid(message: &str) -> FormatError {
    FormatError::new(K::MalformedArchive, message)
}
fn put16(b: &mut Vec<u8>, n: u16) {
    b.extend(n.to_le_bytes());
}
fn put32(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_le_bytes());
}
fn put64(b: &mut Vec<u8>, n: u64) {
    b.extend(n.to_le_bytes());
}
fn read_retry(input: &mut impl Read, bytes: &mut [u8]) -> io::Result<usize> {
    loop {
        match input.read(bytes) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            r => return r,
        }
    }
}
impl<W: Write> StoredZipWriter<W> {
    pub(crate) fn new(output: W) -> Self {
        Self {
            output,
            offset: 0,
            records: Vec::new(),
            names: BTreeSet::new(),
            failed: false,
        }
    }
    fn emit(&mut self, bytes: &[u8]) -> FormatResult<()> {
        let next = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("ZIP64 offset overflow"))?;
        self.output.write_all(bytes)?;
        self.offset = next;
        Ok(())
    }
    pub(crate) fn write_entry(
        &mut self,
        path: &str,
        input: &mut impl Read,
        expected_len: u64,
    ) -> FormatResult<EntryReceipt> {
        if self.failed {
            return Err(invalid("writer cannot continue after an entry failure"));
        }
        // A failed entry leaves an incomplete archive which must never be published.
        self.failed = true;
        check_path(path)?;
        if self.records.len() >= MAX_ENTRIES {
            return Err(FormatError::new(K::ResourceLimit, "too many entries"));
        }
        if !self.names.insert(path.to_owned()) {
            return Err(FormatError::new(K::DuplicateEntry, "duplicate writer path").at(path));
        }
        let offset = self.offset;
        let mut h = Vec::with_capacity(82);
        put32(&mut h, 0x04034b50);
        put16(&mut h, 45);
        put16(&mut h, 8);
        put16(&mut h, 0);
        put32(&mut h, 0);
        put32(&mut h, 0);
        put32(&mut h, u32::MAX);
        put32(&mut h, u32::MAX);
        put16(&mut h, path.len() as u16);
        put16(&mut h, 20);
        h.extend(path.as_bytes());
        put16(&mut h, 1);
        put16(&mut h, 16);
        put64(&mut h, 0);
        put64(&mut h, 0);
        self.emit(&h)?;
        let mut remaining = expected_len;
        let mut sha = Sha256::new();
        let mut crc = u32::MAX;
        let mut buffer = [0; 64 * 1024];
        while remaining > 0 {
            let want = remaining.min(buffer.len() as u64) as usize;
            let n = read_retry(input, &mut buffer[..want])?;
            if n == 0 {
                return Err(
                    FormatError::new(K::IntegrityMismatch, "source shortened during copy").at(path),
                );
            }
            self.emit(&buffer[..n])?;
            sha.update(&buffer[..n]);
            crc = crc_update(crc, &buffer[..n]);
            remaining -= n as u64;
        }
        if read_retry(input, &mut buffer[..1])? != 0 {
            return Err(FormatError::new(K::IntegrityMismatch, "source grew during copy").at(path));
        }
        let crc = !crc;
        let mut descriptor = Vec::with_capacity(24);
        put32(&mut descriptor, 0x08074b50);
        put32(&mut descriptor, crc);
        put64(&mut descriptor, expected_len);
        put64(&mut descriptor, expected_len);
        self.emit(&descriptor)?;
        let sha256 = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
        self.records.push(Record {
            name: path.to_owned(),
            crc,
            size: expected_len,
            offset,
        });
        self.failed = false;
        Ok(EntryReceipt {
            record: IntegrityEntry {
                path: path.to_owned(),
                size_bytes: expected_len,
                sha256,
            },
        })
    }
    pub(crate) fn finish(mut self) -> FormatResult<(W, u64)> {
        if self.failed {
            return Err(invalid("cannot finish a failed writer"));
        }
        let count = self.records.len() as u64;
        if !(3..=MAX_ENTRIES as u64).contains(&count) {
            return Err(invalid("invalid VDISC entry count"));
        }
        let central = self.offset;
        for record in std::mem::take(&mut self.records) {
            self.emit(&central_record(&record))?;
        }
        let size = self
            .offset
            .checked_sub(central)
            .ok_or_else(|| invalid("directory offset overflow"))?;
        let end = self.offset;
        self.emit(&end_records(count, central, size, end))?;
        self.output.flush()?;
        Ok((self.output, self.offset))
    }
}
fn central_record(r: &Record) -> Vec<u8> {
    let mut h = Vec::with_capacity(106);
    put32(&mut h, 0x02014b50);
    put16(&mut h, 45);
    put16(&mut h, 45);
    put16(&mut h, 8);
    put16(&mut h, 0);
    put32(&mut h, 0);
    put32(&mut h, r.crc);
    put32(&mut h, u32::MAX);
    put32(&mut h, u32::MAX);
    put16(&mut h, r.name.len() as u16);
    put16(&mut h, 28);
    put16(&mut h, 0);
    put16(&mut h, 0);
    put16(&mut h, 0);
    put32(&mut h, 0);
    put32(&mut h, u32::MAX);
    h.extend(r.name.as_bytes());
    put16(&mut h, 1);
    put16(&mut h, 24);
    put64(&mut h, r.size);
    put64(&mut h, r.size);
    put64(&mut h, r.offset);
    h
}
fn end_records(count: u64, central: u64, size: u64, end: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(98);
    put32(&mut b, 0x06064b50);
    put64(&mut b, 44);
    put16(&mut b, 45);
    put16(&mut b, 45);
    put32(&mut b, 0);
    put32(&mut b, 0);
    put64(&mut b, count);
    put64(&mut b, count);
    put64(&mut b, size);
    put64(&mut b, central);
    put32(&mut b, 0x07064b50);
    put32(&mut b, 0);
    put64(&mut b, end);
    put32(&mut b, 1);
    put32(&mut b, 0x06054b50);
    put16(&mut b, 0);
    put16(&mut b, 0);
    put16(&mut b, u16::MAX);
    put16(&mut b, u16::MAX);
    put32(&mut b, u32::MAX);
    put32(&mut b, u32::MAX);
    put16(&mut b, 0);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    struct ShortWrite(Vec<u8>);
    impl Write for ShortWrite {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            let n = b.len().min(3);
            self.0.extend(&b[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn partial_writes_emit_consistent_archive_and_receipt() {
        let mut writer = StoredZipWriter::new(ShortWrite(Vec::new()));
        let receipt = writer
            .write_entry("manifest.json", &mut Cursor::new(b"abc"), 3)
            .unwrap();
        assert_eq!(
            receipt.record.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        for path in ["tracks/01.wav", "integrity.json"] {
            writer
                .write_entry(path, &mut Cursor::new(b"1234"), 4)
                .unwrap();
        }
        let (bytes, count) = writer.finish().unwrap();
        assert_eq!(count, bytes.0.len() as u64);
        let entries = super::super::zip::index(&mut Cursor::new(&bytes.0)).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].size, 3);
        assert_eq!(
            &bytes.0[entries[0].data as usize..entries[0].data as usize + 3],
            b"abc"
        );
    }
    #[test]
    fn source_growth_truncation_and_poisoned_finish_are_rejected() {
        for declared in [2, 4] {
            let mut writer = StoredZipWriter::new(Vec::new());
            assert_eq!(
                writer
                    .write_entry("manifest.json", &mut Cursor::new(b"abc"), declared)
                    .unwrap_err()
                    .kind,
                K::IntegrityMismatch
            );
            assert!(writer.finish().is_err());
        }
    }
    #[test]
    fn duplicate_paths_are_rejected() {
        let mut writer = StoredZipWriter::new(Vec::new());
        writer
            .write_entry("manifest.json", &mut Cursor::new(b"x"), 1)
            .unwrap();
        assert_eq!(
            writer
                .write_entry("manifest.json", &mut Cursor::new(b"x"), 1)
                .unwrap_err()
                .kind,
            K::DuplicateEntry
        );
    }
    #[test]
    fn zip64_records_preserve_large_offsets_and_reject_overflow() {
        let large = u64::from(u32::MAX) + 1000;
        let r = Record {
            name: "tracks/01.wav".into(),
            crc: 0,
            size: large,
            offset: large + 500,
        };
        let bytes = central_record(&r);
        let p = 46 + r.name.len() + 4;
        assert_eq!(
            u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap()),
            large
        );
        assert_eq!(
            u64::from_le_bytes(bytes[p + 16..p + 24].try_into().unwrap()),
            large + 500
        );
        let end = end_records(3, large, 300, large + 300);
        assert_eq!(u64::from_le_bytes(end[48..56].try_into().unwrap()), large);
        assert_eq!(
            u64::from_le_bytes(end[64..72].try_into().unwrap()),
            large + 300
        );
        let mut writer = StoredZipWriter::new(io::sink());
        writer.offset = u64::MAX;
        assert!(
            writer
                .write_entry("manifest.json", &mut Cursor::new(b"x"), 1)
                .is_err()
        );
    }
    struct FailWrite {
        left: usize,
    }
    impl Write for FailWrite {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.left == 0 {
                return Err(io::Error::other("simulated full disk"));
            }
            let n = self.left.min(bytes.len());
            self.left -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn disk_failure_poisoning_prevents_finish() {
        let mut writer = StoredZipWriter::new(FailWrite { left: 70 });
        assert!(
            writer
                .write_entry("tracks/01.wav", &mut Cursor::new(vec![0; 100]), 100)
                .is_err()
        );
        assert!(writer.finish().is_err());
    }
    struct Interrupted(bool, Cursor<Vec<u8>>);
    impl Read for Interrupted {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if !self.0 {
                self.0 = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.1.read(bytes)
        }
    }
    #[test]
    fn interrupted_source_read_is_retried() {
        let mut writer = StoredZipWriter::new(Vec::new());
        assert!(
            writer
                .write_entry(
                    "manifest.json",
                    &mut Interrupted(false, Cursor::new(b"abc".to_vec())),
                    3
                )
                .is_ok()
        );
    }
}
