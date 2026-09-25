# VDISC V0.1 — Format and Appearance Migration Specification

Date: 2026-09-25  
Status: Design for review; not an implementation-completion claim.  
Campaign: 01, Objective 12, with the agreed appearance prerequisite.  
Repository baseline: `fikryfauzn/spinny`, commit `bf1d43016a63145aa233dd75de3e906d18fe2200`.

## 1. Intent and scope

A finished virtual CD is a portable, immutable snapshot of one to six ordered local audio tracks and its appearance. It must remain inspectable and playable through a compatible application after the draft and original source files disappear. Burner creates the object; Player consumes it; future Shelf state lives separately.

This specification defines the V1 on-disk contract, validation, appearance expansion, migration, and acceptance criteria. It does not implement the production burner, public reader, player, Shelf, lending, networking, Spotify, UI, or other platforms. Those remain later campaign work. Objective 12 includes fixtures and a reusable format validator; Objective 13 supplies the production writer and Objective 15 the supported consumer API.

The approved decisions are ZIP Stored entries with ZIP64 support, fresh burn identities, strict schemas, original payload preservation, SHA-256 integrity, the limits below, and expanded appearance with a mandatory white-default base color. Details newly made concrete here—exact JSON spelling, draft version 2, codec normalization, strict ZIP profile, and decoder budget—are part of this written review, not claims of earlier individual approval.

## 2. Identity and lifecycle

- `disc_id` is a fresh UUID v4 for each burn attempt that successfully publishes a disc. Use lowercase, hyphenated canonical UUID text. An unpublished failed attempt may discard an allocated ID.
- Burning an unchanged draft twice produces different disc IDs. Copying or renaming a finished file preserves its ID.
- The draft retains its separate identity. Neither draft ID nor draft track IDs appear in the finished manifest.
- `burned_at_unix` records the finalization attempt's time in whole Unix seconds, sampled before manifest construction. It is not the exact filesystem publication instant and need not be unique or monotonic. Reject a system clock before the Unix epoch at creation.
- Finished objects have no mutation API. Playback position, statistics, ownership, shelf location, and lending state are external.
- A disc ID is an identifier, not proof of uniqueness across untrusted files, ownership, or authenticity.

## 3. Container profile

The output filename ends in `.vdisc`; its bytes form a single-volume ZIP archive. Every file uses compression method 0 (Stored), including JSON. Original audio and artwork bytes are embedded unchanged. ZIP64 is permitted when required by sizes or offsets; readers also accept a valid ZIP64 representation of small entries.

The archive contains exactly:

| Entry | Count |
| --- | --- |
| `manifest.json` | 1 |
| `integrity.json` | 1 |
| `tracks/01.<ext>` through `tracks/NN.<ext>` | 1–6, consecutive |
| `artwork/disc.<ext>` | 0–1, iff referenced by appearance |

Writer order is manifest, ordered tracks, optional artwork, integrity. Reader acceptance does not depend on physical entry order. Manifest array order is the sole playback order.

All entry names are exact ASCII strings from these naming rules, case-sensitive, using `/`. Reject absolute paths, drive prefixes, backslashes, NUL, `.` or `..` segments, repeated separators, leading/trailing separators, and alternative names. Do not normalize an invalid name into a valid one. No explicit directory entries, symlinks, special files, duplicate entry names, or unreferenced files are allowed.

Reject encryption, split/multi-volume archives, unsupported compression, self-extracting prefixes, trailing bytes beyond the end record, and ZIP comments. Permit only ZIP64 extra fields in this V1 profile; they must be well-formed and unambiguous. Permit only the UTF-8 filename flag and the data-descriptor flag; all other general-purpose flag bits must be zero. Data descriptors are permitted only when their values agree with the central directory and actual payload. Platform attributes may describe ordinary file permissions but must not describe links, directories, or special files.

Validate local headers against the central directory. Reject conflicting names, methods, flags, CRCs, sizes, offsets, overlapping entry regions, unindexed local entries, and records extending outside the file. When descriptor mode is used, permitted placeholder local sizes/CRCs are not conflicts; the descriptor and central values must agree. ZIP CRC32 checks remain required in addition to SHA-256.

There is no product audio-byte or duration cap beyond representable ZIP64/u64 values and actual storage availability. Use checked arithmetic and bounded streaming; never allocate from an untrusted audio size. Platform/resource inability must return a controlled resource error, not silently reinterpret a valid disc.

## 4. JSON rules

Both JSON entries are UTF-8 objects without a byte-order mark. Reject invalid UTF-8, duplicate object keys at every depth, trailing non-whitespace, unknown fields, missing required fields, wrong types, and `null`. Optional fields are omitted when absent. Booleans are not numbers. Integer fields use nonnegative JSON integer tokens without fractions or exponents and must fit their specified unsigned type.

Object key order and whitespace are not semantically significant, but their exact bytes are significant to integrity hashes. No parse-and-reserialize step is used for hashing. No Unicode normalization is imposed. A supplied text field must contain at least one non-whitespace character; retain its stored text otherwise. JSON file limits bound aggregate text size.

Check a well-formed integer `format_version` before binding the rest of the manifest to V1. Any integer version other than 1 returns UnsupportedVersion rather than a misleading V1 unknown-field error. Malformed JSON or a missing/wrongly typed version remains a schema error.

## 5. Manifest schema

| Field | Required | Type / rule |
| --- | --- | --- |
| `format_version` | Yes | Integer, exactly 1 |
| `disc_id` | Yes | Canonical UUID v4 string |
| `title` | Yes | Nonblank string |
| `burned_at_unix` | Yes | u64 Unix seconds |
| `tracks` | Yes | Array of 1–6 track objects |
| `appearance` | Yes | Appearance object |

No capacity field is serialized: six is a V1 rule. No original local paths, original filenames, draft IDs, source fingerprints, output destinations, or import diagnostics are manifest fields. Preserving original payload bytes also preserves any tags already inside those bytes; this is not a metadata-sanitization format.

### 5.1 Track object

| Field | Required | Type / rule |
| --- | --- | --- |
| `path` | Yes | Canonical path for its 1-based array position |
| `container` | Yes | Canonical value from the table below |
| `codec` | Yes | Canonical value from the table below |
| `duration_ms` | No | u64; 0 allowed for a positive sub-millisecond duration rounded to milliseconds |
| `sample_rate_hz` | No | u32, greater than zero |
| `channels` | No | u16, greater than zero |
| `title`, `artist`, `album`, `genre` | No | Nonblank strings |
| `source_track_number`, `source_track_total` | No | u32 |
| `source_disc_number`, `source_disc_total` | No | u32 |

Source numbering is descriptive metadata. Preserve it without enforcing number ≤ total or positive values: the current importer exposes those values without those constraints. It never controls playback position. Technical values remain absent if unavailable; do not invent zeros. Missing technical metadata alone does not invalidate playable audio.

| Supported payload | `container` | `codec` | Extension |
| --- | --- | --- | --- |
| Native FLAC | `flac` | `flac` | `.flac` |
| MPEG Layer III audio | `mp3` | `mp3` | `.mp3` |
| Ogg Opus | `ogg` | `opus` | `.opus` |
| PCM WAV supported by the decoder | `wav` | `pcm` | `.wav` |

Normalize importer `mpa` to `mp3` only when the detected codec is MP3; normalize `wave` to `wav`. Never select extension from source filename alone. Other pairs are unsupported V1 media, even if a library happens to decode them. The existing WAV fallback must not mislabel an unrelated codec as PCM; verify the actual codec belongs to the supported PCM family when implementing this mapping.

Repeated songs have separate slots and separate payload entries. Do not deduplicate. Disc ID plus 1-based position identifies a slot; no track UUID is serialized.

### 5.2 Appearance object

| Field | Required | Type / rule |
| --- | --- | --- |
| `base_color` | Yes | Object with exactly `r`, `g`, `b`, each u8 |
| `label` | No | Nonblank string |
| `subtitle` | No | Nonblank string |
| `image` | No | Artwork object |

White is `{ "r": 255, "g": 255, "b": 255 }`. A V1 finished manifest missing `base_color` is invalid; the default applies when creating/migrating drafts, not when repairing a malformed finished disc. Title identifies the disc in listings; label is its printed text. No implicit title-to-label fallback is stored.

Artwork has exactly these required fields: `path` (canonical archive path), `format` (`png`, `jpeg`, or `webp`), `width` (u32), `height` (u32). Canonical paths are `artwork/disc.png`, `artwork/disc.jpeg`, and `artwork/disc.webp`. Dimensions must match the decoded image and satisfy section 7. Source paths and source fingerprints stay in drafts only.

Base color and image are independent. Setting/clearing either must preserve the other. Clearing color resets white. Label and subtitle can be cleared independently. Rendering, fonts, crop placement, material, and case design are outside this format.

### 5.3 Illustrative manifest

This is a schema example, not a verified archive fixture; the referenced payload is not supplied by this document.

```json
{
  "format_version": 1,
  "disc_id": "c08971ac-21ef-4f8f-a164-e10b5badc909",
  "title": "Night Drive",
  "burned_at_unix": 1790301600,
  "tracks": [
    {
      "path": "tracks/01.flac",
      "container": "flac",
      "codec": "flac",
      "duration_ms": 214000,
      "sample_rate_hz": 48000,
      "channels": 2,
      "title": "Evening Signal",
      "artist": "Example Artist",
      "source_track_number": 9
    }
  ],
  "appearance": {
    "base_color": { "r": 255, "g": 255, "b": 255 },
    "label": "Night Drive",
    "subtitle": "Volume One"
  }
}
```

## 6. Integrity schema and guarantees

The root object contains exactly `algorithm` (string `sha256`) and `entries` (array). Each entry contains exactly `path` (string), `size_bytes` (u64), and `sha256` (64 lowercase hexadecimal characters).

Entries cover `manifest.json`, every track, and optional artwork exactly once. `integrity.json` is excluded to avoid self-reference. Its array has 2–8 entries. Writers use manifest, tracks, artwork order; readers do not depend on integrity array order.

Compute hashes from the exact uncompressed entry contents. Compare actual streamed byte counts against both ZIP sizes and integrity sizes. Integrity paths must equal the manifest-derived content set plus `manifest.json`. No missing or additional records are allowed. A file named only by integrity is not permitted content. Hashes exclude ZIP headers, timestamps, comments, and other container metadata; structural validation still rejects invalid container metadata.

This detects corruption and inconsistent edits. It does not authenticate the author or prevent someone replacing content and recomputing hashes. No signing, encryption, DRM, or enforced scarcity is claimed. Byte-for-byte copies are valid.

## 7. Limits and resource behavior

Limits are inclusive. MiB and KiB use powers of 1024.

| Item | Maximum |
| --- | ---: |
| `manifest.json` actual bytes | 1,048,576 |
| `integrity.json` actual bytes | 65,536 |
| Encoded artwork bytes | 20,971,520 |
| Image width or height | 8,192 |
| Image pixel count (`width × height`) | 16,000,000 |
| ZIP entries | 9 |
| Tracks | 6 |

Image dimensions must also be nonzero. Enforce both per-side and total-pixel limits with checked multiplication. Bound entry enumeration and JSON reads before collecting all data; reject lengths above limits before allocation and also enforce actual streamed lengths.

Implementation policy proposed for review: a 256 MiB per-image decoder allocation budget. This is a resource ceiling, not a promise that dimension checks bound all decoder memory. Inspect dimensions before full decoding and use enforceable decoder limits; use an isolated decoding worker if the chosen library cannot enforce the budget. A resource failure is explicit and leaves state unchanged. Do not silently resize or transcode artwork to pass limits. Animation is not a promised display feature; preserve source bytes and validate at least the primary decoded image and dimensions.

## 8. Draft appearance migration

The reviewed repository uses draft version 1, a `surface` tagged enum (`none`, `color`, `image`), and an optional label. This approved expansion changes those semantics.

Use draft version 2 for newly written drafts. Load version 1 through a dedicated legacy representation, convert in memory, and validate before exposing the current model. Reading alone must not rewrite the file. On a later successful save, atomically write version 2. Failed loading, migration, or mutation leaves the original bytes intact. Unknown draft versions fail clearly.

| Legacy data | Version 2 result |
| --- | --- |
| No appearance or `surface: none` | White base; no image |
| Color surface | Preserve RGB; no image |
| Image surface | White base; preserve full draft image record |
| Label | Preserve if present |
| Subtitle | Absent |

Retain draft ID, creation time, title, track order, track IDs, tags, paths, and fingerprints. Version 2 appearance has required `base_color`, optional `image`, optional `label`, optional `subtitle`; draft image retains its local source path and fingerprint. Version 2 missing required color is invalid. Legacy null optional labels may be accepted by the legacy loader and converted to absence; this does not relax finished-disc JSON rules.

Add independent color reset, image clear, label clear, and subtitle clear operations. If `clear_disc_surface()` is retained for compatibility, document that it resets white and removes image while preserving label/subtitle. Replace tests asserting color/image mutual exclusion with coexistence tests. Preflight must enforce the new appearance and resource rules.

## 9. Validation architecture and errors

Keep persisted draft types separate from finished-format types; never serialize DraftDisc as a manifest. A conversion boundary drops source paths and fingerprints, normalizes media names, and produces a manifest with the new burn identity.

Use shared pure schema/invariant validation, a bounded archive validator over seekable bytes/files, and reusable streaming hashing. Objective 12 fixtures invoke the same format validation intended for temporary-burn verification and the later public reader. They must not require an already implemented production burner.

Validation sequence:

1. Check ZIP end records, entry count, bounds, names, methods, flags, duplicate names, and header consistency without trusting unbounded counts.
2. Read bounded JSON entries; reject duplicate keys; establish version and validate schemas.
3. Derive the exact allowed payload set and canonical names from the manifest.
4. Compare archive and integrity inventories and sizes.
5. Stream every covered entry to verify SHA-256, byte count, and ZIP CRC32.
6. Validate artwork content and dimensions under limits. Confirm audio container/codec agrees with its declaration through bounded media probing where possible; missing technical metadata remains permitted.
7. Return a validated result only when all required checks pass.

Full-track decoding is not implied by hash validation or a successful initial audio probe. The current importer decodes an initial packet; later playback must still handle decoder failures safely. Stored metadata must not be treated as trusted merely because its manifest hash matches.

Provide structured error categories for unsupported version, invalid JSON/schema, unsafe path, duplicate entry/key, missing/unexpected entry, unsupported ZIP feature, malformed archive, size/resource limit, integrity mismatch, invalid artwork, unsupported/mismatched media, and I/O. Include the entry path when applicable. Return controlled errors; no panics, extraction outside the archive, mutation, or partially exposed playable state. A verifier may stop at the first fatal structural fault; preflight's collection of independent draft issues is separate.

Do not extract archive paths into the filesystem for validation. Later audio adapters may expose a bounded seekable entry view or managed temporary payload, with no consumer knowledge of ZIP internals.

## 10. Acceptance matrix

These are required tests, not results claimed by this document.

| Area | Acceptance cases |
| --- | --- |
| Valid fixtures | One-track and six-track V1; all four audio mappings; mixed formats; optional tags absent; Unicode text |
| Identity | Valid v4 accepted; malformed/non-v4 ID rejected; future burner creates distinct IDs across burns; copying preserves identity |
| Ordering | Array order preserved; canonical names required; physical ZIP and integrity-record ordering may differ |
| Duplicated song | Two separate paths with identical payload bytes accepted |
| JSON | Missing fields, unknown fields, nulls, duplicate keys at every depth, wrong types, overflow, invalid UTF-8, and trailing garbage rejected |
| Version | Unknown integer version produces UnsupportedVersion even with future-shaped fields; missing/wrong version type rejected |
| Inventory | Missing manifest/integrity/audio/artwork, duplicate manifest or payload, extra files, extra/missing hash records rejected |
| Paths | Absolute, traversal, backslash, repeated separator, alternate case, NUL, and mismatched position names rejected |
| ZIP structure | Compression/encryption, links/directories, split archives, bad offsets, overlap, conflicting headers, corrupt CRC, prefixes/trailers, comments, unsupported extras rejected |
| ZIP64 | Valid small ZIP64 fixture accepted; malformed ZIP64 rejected; exercise >4 GiB offsets via suitable sparse/seekable test infrastructure without allocating a giant in-memory buffer |
| Integrity | Changed manifest/audio/artwork bytes rejected; wrong sizes and malformed hashes rejected; exact-byte hashing confirmed |
| Appearance | Default white; simultaneous color/image; subtitle; independent clearing; PNG/JPEG/WebP; mismatch and malformed images rejected |
| Limits | Exactly-at-limit accepted when otherwise valid; one-over rejected; multiplication overflow handled; bounded reads and decoder resource errors |
| Migration | Legacy blank/color/image/label drafts mapped correctly; IDs and track metadata preserved; read does not rewrite; successful save emits version 2; failure is nonmutating |
| Isolation | Fixture verification succeeds without draft or original files; source bytes remain untouched |

Run `cargo build`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --check` as the implementation gate. Required system audio dependencies must be present; inability to run a gate is reported rather than counted as passing.

Objective 12 completion requires the approved specification, appearance prerequisite, reusable format validation, valid/corrupt fixtures, and passing tests. Burner publication, burn-twice identity behavior, and deleting originals after a real burn are later integration gates, not reasons to implement Objectives 13–15 prematurely.

## 11. Boundaries for subsequent objectives

Objective 13 must run preflight internally, generate identity/time, stream-copy and hash the bytes actually copied, compare against imported source fingerprints, verify its temporary artifact, and publish atomically without overwriting an existing destination. Preflight alone cannot prevent a source changing during copying. Failure must not leave a final partial disc.

Objective 14 enforces domain immutability; it cannot make arbitrary filesystem writes impossible. Objective 15 exposes one supported reader and safe payload access, reusing the format validator. Player state and actual sound remain Objectives 16–17.

No case/booklet/sticker fields, track transcoding, UI layout, accounts, lending protocol, networking, or Spotify metadata are added here. Any later format extension requiring new fields uses a new integer format version; future readers may retain explicit V1 support.

## 12. Sources and review status

Requirements: `VDISC_GRAND_SCHEME.md` and `VDISC_CAMPAIGN_01_LINUX_BACKEND.md`, read during this discussion. User decisions in this discussion supersede their older appearance model.

Repository inspected at the pinned baseline:

- [Draft and timestamp conventions](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/draft.rs)
- [Track fields](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/track.rs)
- [Appearance model](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/appearance.rs)
- [Customization operations](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/customization.rs)
- [Audio detection](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/audio.rs)
- [Preflight](https://github.com/fikryfauzn/spinny/blob/bf1d43016a63145aa233dd75de3e906d18fe2200/crates/vdisc-core/src/preflight.rs)

This document has been checked for consistency with the agreed decisions and the inspected code. No product source was changed and no Rust tests were executed as part of writing it. Review this specification before producing the implementation plan.
