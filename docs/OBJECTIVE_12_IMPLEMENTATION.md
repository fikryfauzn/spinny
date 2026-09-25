# Objective 12 implementation

This increment follows the already-applied appearance-v2 prerequisite. It supplies the V1 format contract and reusable validation. Objective 12 remains pending the Rust verification gates below; this source snapshot was reviewed without an available Rust toolchain.

## Contract and behavior

`vdisc_core::format` exposes strict manifest/integrity models, bounded JSON parsing, canonical path checks, limits, a pure `Manifest::from_draft` projection, and `validate_vdisc(path)`. Validation is read-only and checks Stored ZIP/ZIP64 structure, exact inventory, local/central headers, descriptors, CRC32, SHA-256, artwork and declared audio types. Hashing streams in 64 KiB chunks. Validation describes the bytes read during the pass; it cannot guarantee a file remains unchanged afterward.

A mandatory RGB base defaults to white. Artwork, label and subtitle remain independent. Appearance-v2 migration is the prerequisite and is not repeated here. Preflight projects a discarded manifest to catch unsupported codecs, invalid metadata and the 1 MiB manifest ceiling. Projection preserves user metadata and omits draft IDs, track IDs, source paths and source fingerprints. The later burner must supply a fresh UUIDv4 for each successful burn.

Audio verification probes the actual container and codec and decodes an initial packet. It does not certify that every later packet will decode. No total audio byte or duration limit is introduced.

## Linux workers

Build and distribute `vdisc-image-check` and `vdisc-media-check` alongside the application. Cargo builds these package binary targets for integration tests. For normal development, `cargo build --workspace` builds the application and both workers. Building/installing only the CLI package does not install these core-package helper binaries.

Both workers set hard and soft address-space limits to 256 MiB and CPU limits to 10 seconds before parsing input. The image budget covers encoded bytes, decoder state and decoded pixels; an otherwise well-formed file may exceed process resources and receive a controlled resource error. The audio worker uses the same process budget without allocating the track's declared length. It reads an entry view of the original open file through Linux `/proc/<parent>/fd/<fd>`. Linux with mounted `/proc` is required. Other platforms fail explicitly until platform-specific resource enforcement is implemented.

Missing, failed or resource-exhausted workers produce `ResourceLimit`. Optional `VDISC_IMAGE_WORKER` and `VDISC_MEDIA_WORKER` environment variables select trusted helper executables; leave them unset for normal use. These limits contain decoder resource failures; they are not a syscall sandbox.

## Fixtures and coverage

`tests/fixtures/vdisc/` contains two valid and six deliberately invalid archives. They contain synthetic silent PCM, with no personal media. `cargo run -p vdisc-core --example generate_v1_fixtures` regenerates them deterministically using the test-only ZIP writer. The writer is not a production burn API.

New tests cover strict JSON, metadata, path safety, inventory, ZIP64, descriptor widths, malformed headers, corruption, artwork bounds, source independence, draft projection and worker limits/failures. A synthetic seekable fixture exercises offsets beyond 4 GiB without giant allocations or disk writes. Mixed-codec tests reuse the repository's existing `tests/fixtures/audio/valid.{flac,mp3,opus,wav}` files.

The static fixture ZIPs and CRCs were independently checked with Python's ZIP reader; the valid archives' manifest/payload SHA-256 records matched. That check is not a substitute for running the Rust validator.

## Required local gates

Run from the repository root, after applying all changes. Tests were added after implementation, as agreed. Stop and fix any failing gate before committing or advancing to Objective 13.

```bash
cargo fmt
cargo fmt --check
cargo test -p vdisc-core --lib \
  --test vdisc_schema --test vdisc_archive --test vdisc_limits --test vdisc_workers \
  --test disc_appearance --test disc_appearance_v2 --test preflight
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

No Rust build, test, formatting or Clippy success is claimed for this snapshot. Production burn publication, burn-twice identity integration and the public reader remain Objectives 13–15.
