#![allow(dead_code)]
//! Test-only fixture writer, deliberately separate from the production validator.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn wav() -> Vec<u8> {
    let data_len = 960u32;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + data_len).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(data_len.to_le_bytes());
    b.resize(44 + data_len as usize, 0);
    b
}
pub fn manifest() -> Value {
    json!({"format_version":1,"disc_id":"c08971ac-21ef-4f8f-a164-e10b5badc909","title":"Night Drive","burned_at_unix":1790301600u64,
    "tracks":[{"path":"tracks/01.wav","container":"wav","codec":"pcm","duration_ms":10,"sample_rate_hz":48000,"channels":1}],
    "appearance":{"base_color":{"r":255,"g":255,"b":255}}})
}
pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn crc(bytes: &[u8]) -> u32 {
    let mut c = u32::MAX;
    for &b in bytes {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}
pub fn entries(manifest: Value, mut payloads: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    let mut entries = vec![(
        "manifest.json".to_owned(),
        serde_json::to_vec(&manifest).unwrap(),
    )];
    entries.append(&mut payloads);
    let records: Vec<Value> = entries
        .iter()
        .map(|(p, b)| json!({"path":p,"size_bytes":b.len(),"sha256":hash(b)}))
        .collect();
    entries.push((
        "integrity.json".into(),
        serde_json::to_vec(&json!({"algorithm":"sha256","entries":records})).unwrap(),
    ));
    entries
}
pub fn minimal_entries() -> Vec<(String, Vec<u8>)> {
    entries(manifest(), vec![("tracks/01.wav".into(), wav())])
}
fn w16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_le_bytes());
}
fn w32(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}
fn w64(out: &mut Vec<u8>, v: u64) {
    out.extend(v.to_le_bytes());
}

pub fn build(entries: &[(String, Vec<u8>)], wide: bool, descriptor: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets = Vec::new();
    let version = if wide { 45 } else { 20 };
    let flags = if descriptor { 8 } else { 0 };
    for (name, data) in entries {
        offsets.push(out.len() as u64);
        w32(&mut out, 0x04034b50);
        w16(&mut out, version);
        w16(&mut out, flags);
        w16(&mut out, 0);
        w32(&mut out, 0);
        w32(&mut out, if descriptor { 0 } else { crc(data) });
        let size = if wide {
            u32::MAX
        } else if descriptor {
            0
        } else {
            data.len() as u32
        };
        w32(&mut out, size);
        w32(&mut out, size);
        w16(&mut out, name.len() as u16);
        w16(&mut out, if wide { 20 } else { 0 });
        out.extend(name.as_bytes());
        if wide {
            w16(&mut out, 1);
            w16(&mut out, 16);
            let n = if descriptor { 0 } else { data.len() as u64 };
            w64(&mut out, n);
            w64(&mut out, n);
        }
        out.extend(data);
        if descriptor {
            w32(&mut out, 0x08074b50);
            w32(&mut out, crc(data));
            if wide {
                w64(&mut out, data.len() as u64);
                w64(&mut out, data.len() as u64);
            } else {
                w32(&mut out, data.len() as u32);
                w32(&mut out, data.len() as u32);
            }
        }
    }
    let central = out.len() as u64;
    for ((name, data), offset) in entries.iter().zip(offsets) {
        w32(&mut out, 0x02014b50);
        w16(&mut out, version);
        w16(&mut out, version);
        w16(&mut out, flags);
        w16(&mut out, 0);
        w32(&mut out, 0);
        w32(&mut out, crc(data));
        let size = if wide { u32::MAX } else { data.len() as u32 };
        w32(&mut out, size);
        w32(&mut out, size);
        w16(&mut out, name.len() as u16);
        w16(&mut out, if wide { 28 } else { 0 });
        w16(&mut out, 0);
        w16(&mut out, 0);
        w16(&mut out, 0);
        w32(&mut out, 0);
        w32(&mut out, if wide { u32::MAX } else { offset as u32 });
        out.extend(name.as_bytes());
        if wide {
            w16(&mut out, 1);
            w16(&mut out, 24);
            w64(&mut out, data.len() as u64);
            w64(&mut out, data.len() as u64);
            w64(&mut out, offset);
        }
    }
    let central_size = out.len() as u64 - central;
    if wide {
        let end = out.len() as u64;
        w32(&mut out, 0x06064b50);
        w64(&mut out, 44);
        w16(&mut out, 45);
        w16(&mut out, 45);
        w32(&mut out, 0);
        w32(&mut out, 0);
        w64(&mut out, entries.len() as u64);
        w64(&mut out, entries.len() as u64);
        w64(&mut out, central_size);
        w64(&mut out, central);
        w32(&mut out, 0x07064b50);
        w32(&mut out, 0);
        w64(&mut out, end);
        w32(&mut out, 1);
    }
    w32(&mut out, 0x06054b50);
    w16(&mut out, 0);
    w16(&mut out, 0);
    let n = if wide { u16::MAX } else { entries.len() as u16 };
    w16(&mut out, n);
    w16(&mut out, n);
    w32(&mut out, if wide { u32::MAX } else { central_size as u32 });
    w32(&mut out, if wide { u32::MAX } else { central as u32 });
    w16(&mut out, 0);
    out
}
pub fn locations(bytes: &[u8], signature: [u8; 4]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == signature).then_some(i))
        .collect()
}
