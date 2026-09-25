//! A bounded parser for the deliberately narrow VDISC Stored ZIP profile.
//! This is not a general-purpose ZIP reader or extractor.
use super::{FormatError, FormatErrorKind as K, FormatResult, MAX_ENTRIES, check_path};
use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
};

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub name: String,
    pub size: u64,
    pub data: u64,
    pub crc: u32,
    local: u64,
    flags: u16,
    version: u16,
    time: u32,
}
fn bad(message: &str) -> FormatError {
    FormatError::new(K::MalformedArchive, message)
}
fn unsupported(message: &str) -> FormatError {
    FormatError::new(K::UnsupportedZipFeature, message)
}
fn add(a: u64, b: u64) -> FormatResult<u64> {
    a.checked_add(b).ok_or_else(|| bad("offset overflow"))
}
fn u16le(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([b[p], b[p + 1]])
}
fn u32le(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}
fn u64le(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().expect("fixed bounded field"))
}
fn fixed<R: Read + Seek, const N: usize>(r: &mut R, offset: u64) -> FormatResult<[u8; N]> {
    r.seek(SeekFrom::Start(offset))?;
    let mut b = [0; N];
    r.read_exact(&mut b).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            bad("truncated ZIP record")
        } else {
            e.into()
        }
    })?;
    Ok(b)
}
fn variable<R: Read>(r: &mut R, n: usize) -> FormatResult<Vec<u8>> {
    let mut b = vec![0; n];
    r.read_exact(&mut b).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            bad("truncated ZIP variable field")
        } else {
            e.into()
        }
    })?;
    Ok(b)
}
fn check_version(version: u16) -> FormatResult<()> {
    if !(10..=45).contains(&version) {
        return Err(unsupported("unsupported ZIP version needed"));
    }
    Ok(())
}
fn check_flags(flags: u16, method: u16) -> FormatResult<()> {
    if flags & !0x0808 != 0 || method != 0 {
        return Err(unsupported(
            "only unencrypted Stored entries with UTF-8/descriptor flags are supported",
        ));
    }
    Ok(())
}
// Only a single ZIP64 extra is allowed, with fields exactly matching sentinels.
fn extended(
    extra: &[u8],
    size: u32,
    compressed: u32,
    offset: Option<u32>,
) -> FormatResult<(u64, u64, Option<u64>)> {
    let need = [
        size == u32::MAX,
        compressed == u32::MAX,
        offset == Some(u32::MAX),
    ];
    let fields = need.iter().filter(|&&x| x).count();
    if fields == 0 {
        if !extra.is_empty() {
            return Err(unsupported("unexpected extra field"));
        }
        return Ok((
            u64::from(size),
            u64::from(compressed),
            offset.map(u64::from),
        ));
    }
    if extra.len() != 4 + fields * 8
        || u16le(extra, 0) != 1
        || usize::from(u16le(extra, 2)) != fields * 8
    {
        return Err(bad("malformed or unsupported ZIP64 extra"));
    }
    let mut p = 4;
    let mut next = |needed: bool, old: u32| {
        if needed {
            let n = u64le(extra, p);
            p += 8;
            n
        } else {
            u64::from(old)
        }
    };
    let s = next(need[0], size);
    let c = next(need[1], compressed);
    let o = offset.map(|old| next(need[2], old));
    Ok((s, c, o))
}
fn classic_agrees(value: u32, wide: u64) -> bool {
    value == u32::MAX || u64::from(value) == wide
}

pub(crate) fn index<R: Read + Seek>(r: &mut R) -> FormatResult<Vec<Entry>> {
    let length = r.seek(SeekFrom::End(0))?;
    if length < 22 {
        return Err(bad("missing ZIP end record"));
    }
    let end = length - 22;
    let e = fixed::<_, 22>(r, end)?;
    if u32le(&e, 0) != 0x06054b50 || u16le(&e, 20) != 0 {
        return Err(bad(
            "end record must be last, with no comment/trailing bytes",
        ));
    }
    if u16le(&e, 4) != 0 || u16le(&e, 6) != 0 || u16le(&e, 8) != u16le(&e, 10) {
        return Err(unsupported("split ZIP archives are not supported"));
    }
    let small_count = u16le(&e, 10);
    let small_size = u32le(&e, 12);
    let small_offset = u32le(&e, 16);
    let locator = if end >= 20 {
        Some(fixed::<_, 20>(r, end - 20)?)
    } else {
        None
    };
    let (count, cd_size, cd_offset, cd_end) =
        if let Some(l) = locator.filter(|l| u32le(l, 0) == 0x07064b50) {
            if u32le(&l, 4) != 0 || u32le(&l, 16) != 1 {
                return Err(unsupported("split ZIP64 archives are not supported"));
            }
            let pos = u64le(&l, 8);
            if add(pos, 56)? != end - 20 {
                return Err(bad("ZIP64 record must directly precede locator"));
            }
            let z = fixed::<_, 56>(r, pos)?;
            if u32le(&z, 0) != 0x06064b50 || u64le(&z, 4) != 44 {
                return Err(bad("invalid ZIP64 end record"));
            }
            check_version(u16le(&z, 14))?;
            if u32le(&z, 16) != 0 || u32le(&z, 20) != 0 || u64le(&z, 24) != u64le(&z, 32) {
                return Err(unsupported("split ZIP64 archive"));
            }
            let n = u64le(&z, 32);
            let size = u64le(&z, 40);
            let off = u64le(&z, 48);
            if (small_count != u16::MAX && u64::from(small_count) != n)
                || !classic_agrees(small_size, size)
                || !classic_agrees(small_offset, off)
            {
                return Err(bad("classic/ZIP64 end records disagree"));
            }
            (n, size, off, pos)
        } else {
            if small_count == u16::MAX || small_size == u32::MAX || small_offset == u32::MAX {
                return Err(bad("missing ZIP64 end records"));
            }
            (
                u64::from(small_count),
                u64::from(small_size),
                u64::from(small_offset),
                end,
            )
        };
    if count > MAX_ENTRIES as u64 {
        return Err(FormatError::new(K::ResourceLimit, "too many ZIP entries"));
    }
    if count < 3 {
        return Err(bad("VDISC requires at least three entries"));
    }
    if cd_size > 8192 {
        return Err(FormatError::new(
            K::ResourceLimit,
            "central directory exceeds V1 bound",
        ));
    }
    if add(cd_offset, cd_size)? != cd_end {
        return Err(bad("central directory bounds disagree"));
    }
    let mut entries = Vec::new();
    let mut names = BTreeSet::new();
    let mut pos = cd_offset;
    for _ in 0..count {
        if add(pos, 46)? > cd_end {
            return Err(bad("truncated central directory"));
        }
        let h = fixed::<_, 46>(r, pos)?;
        if u32le(&h, 0) != 0x02014b50 {
            return Err(bad("invalid central header"));
        }
        let version = u16le(&h, 6);
        check_version(version)?;
        let flags = u16le(&h, 8);
        check_flags(flags, u16le(&h, 10))?;
        let name_len = usize::from(u16le(&h, 28));
        let extra_len = usize::from(u16le(&h, 30));
        if name_len > 32 || extra_len > 28 {
            return Err(FormatError::new(
                K::ResourceLimit,
                "ZIP name/extra length exceeds V1 profile",
            ));
        }
        if u16le(&h, 32) != 0 || u16le(&h, 34) != 0 {
            return Err(unsupported(
                "entry comments and split entries are not supported",
            ));
        }
        let attrs = u32le(&h, 38);
        let mode = (attrs >> 16) & 0xf000;
        if attrs & 0x18 != 0 || (mode != 0 && mode != 0x8000) {
            return Err(unsupported("only regular-file entries are allowed"));
        }
        let next = add(pos, 46 + (name_len + extra_len) as u64)?;
        if next > cd_end {
            return Err(bad("central variable fields outside directory"));
        }
        let name =
            String::from_utf8(variable(r, name_len)?).map_err(|_| bad("non-UTF8 ZIP name"))?;
        check_path(&name)?;
        if !names.insert(name.clone()) {
            return Err(FormatError::new(K::DuplicateEntry, "duplicate archive name").at(&name));
        }
        let extra = variable(r, extra_len)?;
        let (size, compressed, offset) =
            extended(&extra, u32le(&h, 24), u32le(&h, 20), Some(u32le(&h, 42)))?;
        if size != compressed {
            return Err(bad("Stored compressed/uncompressed sizes disagree").at(&name));
        }
        if !extra.is_empty() && version < 45 {
            return Err(bad("ZIP64 entry requires version 45"));
        }
        entries.push(Entry {
            name,
            size,
            data: 0,
            crc: u32le(&h, 16),
            local: offset.expect("central offset is supplied"),
            flags,
            version,
            time: u32le(&h, 12),
        });
        pos = next;
    }
    if pos != cd_end {
        return Err(bad("extra central-directory data"));
    }
    entries.sort_by_key(|e| e.local);
    let mut next = 0;
    for entry in &mut entries {
        if entry.local != next {
            return Err(bad("prefix, gap, overlap, or unindexed local entry").at(&entry.name));
        }
        if add(next, 30)? > cd_offset {
            return Err(bad("local header overlaps central directory"));
        }
        let h = fixed::<_, 30>(r, next)?;
        if u32le(&h, 0) != 0x04034b50
            || u16le(&h, 4) != entry.version
            || u16le(&h, 6) != entry.flags
            || u16le(&h, 8) != 0
            || u32le(&h, 10) != entry.time
        {
            return Err(bad("local/central header disagreement").at(&entry.name));
        }
        let name_len = usize::from(u16le(&h, 26));
        let extra_len = usize::from(u16le(&h, 28));
        if name_len != entry.name.len() || extra_len > 20 {
            return Err(bad("invalid local variable field lengths"));
        }
        let data = add(next, 30 + (name_len + extra_len) as u64)?;
        if data > cd_offset {
            return Err(bad("local fields outside data area"));
        }
        if variable(r, name_len)? != entry.name.as_bytes() {
            return Err(bad("local filename disagreement"));
        }
        let extra = variable(r, extra_len)?;
        let (size, compressed, _) = extended(&extra, u32le(&h, 22), u32le(&h, 18), None)?;
        if !extra.is_empty() && entry.version < 45 {
            return Err(bad("ZIP64 local entry requires version 45"));
        }
        let descriptor = entry.flags & 8 != 0;
        let crc = u32le(&h, 14);
        if descriptor {
            if (size != 0 && size != entry.size)
                || (compressed != 0 && compressed != entry.size)
                || (crc != 0 && crc != entry.crc)
            {
                return Err(bad("invalid descriptor-mode local placeholders"));
            }
        } else if size != entry.size || compressed != entry.size || crc != entry.crc {
            return Err(bad("local size/CRC disagreement"));
        }
        entry.data = data;
        next = add(data, entry.size)?;
        if next > cd_offset {
            return Err(bad("payload outside data area"));
        }
        if descriptor {
            let wide = u32le(&h, 22) == u32::MAX
                || u32le(&h, 18) == u32::MAX
                || entry.size >= u64::from(u32::MAX);
            let n = if wide { 20 } else { 12 };
            if add(next, n as u64)? > cd_offset {
                return Err(bad("truncated data descriptor"));
            }
            let first = fixed::<_, 4>(r, next)?;
            // A CRC equal to the signature is ambiguous; try both legal layouts.
            let mut matched = None;
            for prefix in [0usize, 4] {
                if prefix == 4 && u32le(&first, 0) != 0x08074b50 {
                    continue;
                }
                let end = add(next, (n + prefix) as u64)?;
                if end > cd_offset {
                    continue;
                }
                r.seek(SeekFrom::Start(next + prefix as u64))?;
                let d = variable(r, n)?;
                let (c, s) = if wide {
                    (u64le(&d, 4), u64le(&d, 12))
                } else {
                    (u64::from(u32le(&d, 4)), u64::from(u32le(&d, 8)))
                };
                if u32le(&d, 0) == entry.crc && c == entry.size && s == entry.size {
                    matched = Some(end);
                    break;
                }
            }
            next = matched.ok_or_else(|| bad("data descriptor disagreement"))?;
        }
    }
    if next != cd_offset {
        return Err(bad("unindexed bytes before central directory"));
    }
    Ok(entries)
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut j = 0;
        while j < 8 {
            c = if c & 1 != 0 {
                0xedb88320 ^ (c >> 1)
            } else {
                c >> 1
            };
            j += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}
const CRC_TABLE: [u32; 256] = crc_table();
pub(crate) fn crc_update(mut state: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        state = CRC_TABLE[((state ^ u32::from(b)) & 255) as usize] ^ (state >> 8);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Seekable synthetic sparse file: no 4 GiB allocation or disk write.
    struct Sparse {
        pieces: Vec<(u64, Vec<u8>)>,
        len: u64,
        pos: u64,
    }
    impl Read for Sparse {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = (self.len - self.pos).min(buf.len() as u64) as usize;
            buf[..n].fill(0);
            for (start, bytes) in &self.pieces {
                let lo = self.pos.max(*start);
                let hi = (self.pos + n as u64).min(*start + bytes.len() as u64);
                if hi > lo {
                    buf[(lo - self.pos) as usize..(hi - self.pos) as usize]
                        .copy_from_slice(&bytes[(lo - start) as usize..(hi - start) as usize]);
                }
            }
            self.pos += n as u64;
            Ok(n)
        }
    }
    impl Seek for Sparse {
        fn seek(&mut self, s: SeekFrom) -> std::io::Result<u64> {
            let p = match s {
                SeekFrom::Start(p) => i128::from(p),
                SeekFrom::Current(n) => i128::from(self.pos) + i128::from(n),
                SeekFrom::End(n) => i128::from(self.len) + i128::from(n),
            };
            if p < 0 || p > i128::from(self.len) {
                return Err(std::io::Error::other("bad sparse seek"));
            }
            self.pos = p as u64;
            Ok(self.pos)
        }
    }
    #[test]
    fn crc_matches_standard_vector() {
        assert_eq!(!crc_update(u32::MAX, b"123456789"), 0xcbf43926);
    }
    #[test]
    fn zip64_offsets_beyond_four_gib_are_not_truncated() {
        let names = ["tracks/01.wav", "manifest.json", "integrity.json"];
        let sizes = [u64::from(u32::MAX) + 100, 2, 2];
        let mut pieces = Vec::new();
        let mut offsets = Vec::new();
        let mut pos = 0;
        for (name, size) in names.iter().zip(sizes) {
            offsets.push(pos);
            let mut h = vec![0u8; 30];
            h[..4].copy_from_slice(&0x04034b50u32.to_le_bytes());
            h[4..6].copy_from_slice(&45u16.to_le_bytes());
            h[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
            h[22..26].copy_from_slice(&u32::MAX.to_le_bytes());
            h[26..28].copy_from_slice(&(name.len() as u16).to_le_bytes());
            h[28..30].copy_from_slice(&20u16.to_le_bytes());
            h.extend(name.as_bytes());
            h.extend(1u16.to_le_bytes());
            h.extend(16u16.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend(size.to_le_bytes());
            let next = pos + h.len() as u64 + size;
            pieces.push((pos, h));
            pos = next;
        }
        let central_offset = pos;
        let mut central = Vec::new();
        for ((name, size), offset) in names.iter().zip(sizes).zip(offsets) {
            let mut h = vec![0u8; 46];
            h[..4].copy_from_slice(&0x02014b50u32.to_le_bytes());
            h[4..6].copy_from_slice(&45u16.to_le_bytes());
            h[6..8].copy_from_slice(&45u16.to_le_bytes());
            h[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
            h[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
            h[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
            h[30..32].copy_from_slice(&28u16.to_le_bytes());
            h[42..46].copy_from_slice(&u32::MAX.to_le_bytes());
            h.extend(name.as_bytes());
            h.extend(1u16.to_le_bytes());
            h.extend(24u16.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend(offset.to_le_bytes());
            central.extend(h);
        }
        let central_size = central.len() as u64;
        pieces.push((pos, central));
        pos += central_size;
        let zip64_offset = pos;
        let mut end = vec![0u8; 56];
        end[..4].copy_from_slice(&0x06064b50u32.to_le_bytes());
        end[4..12].copy_from_slice(&44u64.to_le_bytes());
        end[14..16].copy_from_slice(&45u16.to_le_bytes());
        end[24..32].copy_from_slice(&3u64.to_le_bytes());
        end[32..40].copy_from_slice(&3u64.to_le_bytes());
        end[40..48].copy_from_slice(&central_size.to_le_bytes());
        end[48..56].copy_from_slice(&central_offset.to_le_bytes());
        end.extend(0x07064b50u32.to_le_bytes());
        end.extend(0u32.to_le_bytes());
        end.extend(zip64_offset.to_le_bytes());
        end.extend(1u32.to_le_bytes());
        end.extend(0x06054b50u32.to_le_bytes());
        end.extend([0u8; 4]);
        end.extend(u16::MAX.to_le_bytes());
        end.extend(u16::MAX.to_le_bytes());
        end.extend(u32::MAX.to_le_bytes());
        end.extend(u32::MAX.to_le_bytes());
        end.extend(0u16.to_le_bytes());
        let len = pos + end.len() as u64;
        pieces.push((pos, end));
        let mut sparse = Sparse {
            pieces,
            len,
            pos: 0,
        };
        let entries = index(&mut sparse).unwrap();
        assert_eq!(entries[0].size, sizes[0]);
        assert!(entries[1].data > u64::from(u32::MAX));
    }
}
