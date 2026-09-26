# Objective 13 — synchronous burner

Implementation status: source and tests prepared; Rust gates pending local execution. This increment follows the user's Objective 12 commit `dbc5838`.

## API

`vdisc_core::burn(draft_path, output_path)` loads one saved draft snapshot and returns `Result<BurnResult, BurnError>`. It runs preflight, creates a fresh UUIDv4 and Unix-seconds timestamp, copies original payload bytes into Stored ZIP64, validates the complete temporary file, and atomically publishes without replacing an existing destination.

The result exposes `disc_id`, `burned_at_unix`, `output_path`, `track_count`, `size_bytes` and `durability`. Always inspect durability:

```rust
use vdisc_core::{burn, BurnDurability};

let result = burn("mix.vdraft", "mix.vdisc")?;
match &result.durability {
    BurnDurability::Synced => println!("Published {}", result.output_path.display()),
    BurnDurability::PublishedButDirectorySyncFailed { diagnostic } => {
        eprintln!("Published {}, but directory durability is uncertain: {}",
                  result.output_path.display(), diagnostic);
    }
}
# Ok::<(), vdisc_core::BurnError>(())
```

This is a core API. No CLI burn command, progress callback, cancellation, playback or public reader has been added.

## Guarantees and boundaries

- Draft/source files are read-only. The draft is loaded once; subsequent saved-draft edits do not change that snapshot.
- Copied byte counts and SHA-256 are checked against the snapshot's source fingerprints. Copy buffers are 64 KiB; whole tracks are never buffered. A source changing after preflight is rejected when the copied bytes differ.
- ZIP64 is used even for small discs. The shared Objective 12 validator checks every completed artifact before publication.
- File syncing precedes validation/publication. Atomic no-replace rename is the publication point; final directory syncing follows it.
- A published result with uncertain durability means the disc already exists. Do not automatically retry as though the burn failed.
- Prepublication errors attempt cleanup of this burn's temporary file. If cleanup fails, `BurnError.cleanup` identifies the leftover path and diagnostic while preserving the original failure.
- Crash/power loss may leave `.vdisc-burn-<uuid>.tmp`. No automatic sweeping is performed. Remove a leftover manually only when no active burn owns it.
- Parent directory operations remain pinned to an open fd if the parent is renamed. Result/cleanup paths describe names at resolution time; after a rename, locate the original directory rather than assuming the old path still identifies it. Preflight report issue paths can contain the internal `/proc` anchor; the outer error carries the resolved destination.
- Linux, mounted `/proc`, and filesystem support for atomic no-replace rename are required. Unsupported publication fails explicitly without falling back to overwrite behavior.
- Build/distribute both existing `vdisc-image-check` and `vdisc-media-check` helper binaries beside the application (`cargo build --workspace`). Integration-test builds include these helpers. Resource limits are not a syscall sandbox; the local output directory is trusted.

No schema version or migration changes. No additional production dependency.

## Tests

New tests exercise real one/six-track burns, mixed codecs, metadata and image preservation, new identity per burn, independence from originals, changed sources, invalid/occupied outputs (including dangling symlinks), relative/non-UTF8 paths, deterministic publication races, pinned parent rename, temporary corruption, cleanup failure and published-but-unsynced outcomes. Private writer tests cover interrupted/short I/O, growth/truncation, ZIP64 record offsets above 4 GiB and arithmetic overflow.

Tests were added after implementation as requested. Required gate order:

```bash
cargo fmt
cargo fmt --check
cargo test -p vdisc-core --lib --test burn --test preflight \
  --test vdisc_archive --test vdisc_schema --test vdisc_limits --test vdisc_workers
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

No passing Rust results are claimed for this handoff. Review returned local logs before marking Objective 13 complete or committing.
