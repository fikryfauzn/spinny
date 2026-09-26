# Objective 13 Synchronous Burner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects delegated execution. Execute task-by-task; keep tests after implementation as the user explicitly requires.

**Goal:** Publish verified, self-contained VDISC files from saved drafts with synchronous completion and no replacement of existing destinations.

**Architecture:** Preflight and projection share one loaded draft snapshot. A private streaming ZIP64 writer creates a temporary artifact; a Linux directory-anchored publication component owns its lifecycle. The existing format validator gates publication, and the public result distinguishes synced publication from publication with uncertain directory durability.

**Tech Stack:** Existing Rust 2024 workspace, std I/O, serde_json, sha2, uuid and Linux libc. Existing image/media helper binaries remain required. No new production dependencies.

**Spec:** `docs/superpowers/specs/2026-09-25-objective13-burner-design.md`, accepted by the user's “next” response. Read it with this plan.

## Global constraints

- “The function returns after publishing the file or reporting a failure.”
- “It never modifies the draft, source audio or source artwork.”
- “No new product-wide audio size or duration cap.”
- “The draft and disc schemas stay at their current versions; no migration is required.”
- “Do not create missing destination parents.”
- “No fallible operation after that point may be reported as though publication did not happen.”
- Always emit Stored ZIP64 with signed 64-bit descriptors; preserve exact source bytes.
- User sequence: implementation first, tests last, then fmt → fmt check → focused tests → entire workspace tests → Clippy.
- Baseline is the user's local `dbc5838`. The available source snapshot contains the delivered corrections but has not been rustfmt-normalized. Do not claim a local git checkout or toolchain exists unless verified at execution time.
- Deliver full changed files for the user's repository if that remains the available execution route. Do not overwrite the user's newer work or push remotely.

## Review focus

1. Bare relative output filenames: resolve against a captured current directory and retain correct `.vdisc` validation.
2. Dangling destination symlinks: reject them as existing entries, without following/replacing them.
3. Draft file replacement during validation: all per-track checks must continue using the loaded snapshot.
4. Output parent renamed/replaced during a burn: fd-relative operations stay in the opened directory; returned paths are names used at resolution time, not permanent inode locators.
5. Cleanup failure after an earlier failure: retain both errors and the temporary path; never mask the original cause.

All five cases have assigned checks in Task 5.

## File ownership

| Path | Change | Owns |
| --- | --- | --- |
| `crates/vdisc-core/src/preflight.rs` | Modify | Loaded-snapshot validation and existing path wrapper |
| `crates/vdisc-core/src/burn.rs` | Create | Public API and burn transaction |
| `crates/vdisc-core/src/burn/error.rs` | Create | Typed burn phase/error and cleanup diagnostic |
| `crates/vdisc-core/src/burn/publication.rs` | Create | Pinned parent, temp creation, rename, sync and cleanup |
| `crates/vdisc-core/src/format/writer.rs` | Create | Streaming Stored ZIP64 emission |
| `crates/vdisc-core/src/format/mod.rs` | Modify | Register private writer |
| `crates/vdisc-core/src/lib.rs` | Modify | Export public burn types |
| `crates/vdisc-core/tests/burn.rs` | Create | End-to-end integration checks |
| `crates/vdisc-core/tests/preflight.rs` | Modify only if needed | Public preflight regression cases |
| `docs/OBJECTIVE_13_IMPLEMENTATION.md` | Create | Deployment, use, guarantees, verified status |

Tests for private fault seams live in the owning modules under `cfg(test)`. Keep the ZIP parser implementation unchanged: expose its existing CRC routine through `format` with a crate-private re-export only if writer placement requires it. No draft migrations, public reader API or CLI expansion.

## Task 1 — Snapshot preflight

**Consumes:** `DraftDisc`, `PreflightReport`, `TrackSourceSelection::new`, existing audio/metadata validation.

**Produces:**

```rust
pub(crate) fn preflight_snapshot(
    draft: &DraftDisc,
    draft_path: &Path,
    output_path: &Path,
) -> PreflightReport;
```

- [ ] Extract the existing loaded-draft checks into `preflight_snapshot`. Keep load-failure reporting and output validation in the public wrapper consistent; do not double-add destination issues.
- [ ] Pass the snapshot UUID into `validate_track_source`. Replace `AddTrackRequest::begin(draft_path)` (which reloads the file on every track) with the existing crate-private selection constructor:

```rust
let selection = TrackSourceSelection::new(
    draft_path.to_path_buf(), draft.id(), TrackSourceKind::Local,
).select_local_file(track.source_path());
let audio = selection.and_then(ValidatedLocalAudio::validate);
```

- [ ] Preserve actual media and metadata checks and collected preflight issues. Ensure no downstream operation in that chain reloads the draft.
- [ ] Replace destination `exists()` with `symlink_metadata`: success means occupied, NotFound means absent, other failures become an explicit destination inspection issue. Keep `.vdisc`, parent, and read-only checks.
- [ ] Mark snapshot replacement and dangling-symlink checks for Task 5; add no tests yet.

Expected deliverable: callers can validate an already-loaded draft without importing changes from a later disk reload.

## Task 2 — Streaming Stored ZIP64 writer

**Files:** create `format/writer.rs`; register it as `pub(crate) mod writer`.

**Produces:**

```rust
pub(crate) struct EntryReceipt {
    pub record: IntegrityEntry,
}
pub(crate) struct StoredZipWriter<W: Write> { /* bounded directory records, offset, W */ }
// Exact method contracts:
// new(output: W) -> Self
// write_entry(&mut self, path: &str, input: &mut impl Read,
//             expected_len: u64) -> FormatResult<EntryReceipt>
// finish(self) -> FormatResult<(W, u64)>
```

The braces comment above describes private storage, not executable scaffolding. Implement private central records with name, CRC, size and local-header offset.

- [ ] Emit ZIP64 local headers: version 45, method Stored, descriptor flag, zero CRC, classic size sentinels and a ZIP64 extra containing zero placeholder sizes. Use canonical ASCII names and zero DOS times.
- [ ] Copy with a fixed 64 KiB buffer, decrementing the expected remaining length. Reject premature EOF. Retry interrupted reads. Feed SHA-256/CRC only after `write_all` succeeds; track offsets with checked addition.
- [ ] After expected bytes, read one extra byte: EOF accepts the length, any byte rejects growth. Finalize size/hash into `IntegrityEntry`, and emit signed 64-bit descriptor with CRC and both sizes.
- [ ] Enforce at most nine unique canonical names. Write central records with ZIP64 size/offset extras, ZIP64 end record, locator and sentinel classic end record. No comments, gaps or directory entries.
- [ ] Use the same writer for bounded manifest/integrity buffers through `Cursor<&[u8]>`; expose the final writer and total byte count from `finish`.
- [ ] Keep the fixture generator separate. Assign short-write, EOF, extra-byte, duplicate-entry and >4 GiB offset checks to Task 5.

Expected deliverable: private writer can produce the precise profile accepted by Objective 12 without buffering whole audio files.

## Task 3 — Publication and typed results

**Files:** create `burn/error.rs`, `burn/publication.rs`, and declare them from `burn.rs`.

Define public `BurnPhase` variants for Load, Preflight, Clock, Source, Write, FileSync, Verify, Publish. `BurnError` carries phase, path, diagnostic, optional full `PreflightReport`, and optional `CleanupFailure { temporary_path, diagnostic }`. Implement Display and Error; retain useful error context rather than replacing every failure with a generic string.

Define:

```rust
pub enum BurnDurability {
    Synced,
    PublishedButDirectorySyncFailed { diagnostic: String },
}
pub struct BurnResult {
    pub disc_id: Uuid,
    pub burned_at_unix: u64,
    pub output_path: PathBuf,
    pub track_count: usize,
    pub size_bytes: u64,
    pub durability: BurnDurability,
}
```

Internal `Publication` owns the directory File, destination/temp basenames, diagnostic absolute paths, temp File and published flag. Required operations: `prepare(output: &Path)`, `create_temp(&mut self)`, `temp_file_mut(&mut self)`, `validation_path(&self)`, `publish(&mut self)`, `sync_directory(&self)`, `cleanup(&mut self)`.

- [ ] Capture cwd once; resolve relative output, split filename/parent and open parent with Linux directory flags. Convert basenames with `OsStrExt` and checked CString construction, preserving non-UTF8 parent names.
- [ ] Use UUID-based `.vdisc-burn-<uuid>.tmp` names and exclusive `openat` creation with mode 0600 and close-on-exec. Retry name collisions only with a fresh name. Do not open/create the final path for writing.
- [ ] Build the validation path as `/proc/<current-pid>/fd/<parent-fd>/<temp-name>` while keeping directory/temp handles alive.
- [ ] Publish with `renameat2` and `RENAME_NOREPLACE` relative to the same directory fd. Preserve existing names on EEXIST. Unsupported syscall/filesystem behavior is an explicit publication error, with no replacing fallback.
- [ ] Mark published immediately after successful rename. Cleanup uses `unlinkat` only on the owned temporary name and never on the final destination. Explicit cleanup enriches returned errors; Drop provides best-effort backup without masking diagnostics.
- [ ] `sync_directory` runs only after publication and returns a diagnostic on failure. On non-Linux, return an unsupported-platform error before creating output.
- [ ] Add private operation seams sufficient to inject rename, file-sync, directory-sync and unlink failures in Task 5. No public fault flags or global mutable test state.

Expected deliverable: transaction storage correctly distinguishes before/after publication and owns cleanup.

## Task 4 — Burn orchestration

**Consumes:** Tasks 1–3, `Manifest::from_draft`, `validate_vdisc`, stored source fingerprints.

**Produces:**

```rust
pub fn burn(
    draft_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> std::result::Result<BurnResult, BurnError>;
```

- [ ] Load the draft once; prepare/pin the destination parent and run snapshot preflight. Invalid inputs return before temporary creation. Project with `Uuid::new_v4()` and checked `SystemTime::now().duration_since(UNIX_EPOCH)`.
- [ ] Serialize and size-check the manifest. Create temp and instantiate `StoredZipWriter` over its File reference. Write manifest, ordered tracks, optional artwork, then integrity.
- [ ] For each source, open a regular file, compare handle metadata length with expected size and stream through `write_entry`. Compare the resulting receipt's hash/size with the snapshot fingerprint. A mismatch maps to Source phase and triggers cleanup.
- [ ] Include manifest and all payload receipts in integrity, excluding integrity itself. Validate and bound integrity serialization before writing it.
- [ ] Finish and release the writer borrow. Sync the temp File. Run the full existing validator on the pinned temp path; compare its manifest with the intended manifest. Construct all result data before rename.
- [ ] Publish. Return `Synced` after successful directory sync or `PublishedButDirectorySyncFailed` with the diagnostic. Never route the latter through prepublication cleanup/error handling.
- [ ] Export burn/result/error/durability types from `lib.rs`. Document snapshot semantics and mandatory inspection of durability.
- [ ] Write `OBJECTIVE_13_IMPLEMENTATION.md` with invocation example, worker requirements, platform constraints and crash leftovers. Label verification pending until Task 6 passes.

Expected deliverable: complete synchronous burn flow, ready for tests.

## Task 5 — Add tests after all implementation

Add public integration tests in `tests/burn.rs`; private tests in `preflight.rs`, `format/writer.rs` and `burn/publication.rs`/`burn.rs`. Build test drafts through existing import/customization APIs and repository audio fixtures.

- [ ] Basic contract: burn 1 and 6 tracks, mixed codecs, duplicate song selections, order, tag fields, default white and customized appearance. Read results through `validate_vdisc`; compare exact embedded bytes using a test-only archive helper, keeping production reader scope unchanged.
- [ ] Identity: burn the same draft twice and assert distinct UUIDv4 IDs and matching manifest/result timestamps. Delete only test-owned originals/draft and validate both output files again.
- [ ] Failure contract: invalid draft, missing source, stale fingerprint, bad extension and occupied destination leave draft/source bytes unchanged. Assert no temp leftovers for ordinary failures.
- [ ] Review focus 1: use a bare filename in a subprocess with a temporary working directory, avoiding process-global cwd races. Result path is absolute and output is valid.
- [ ] Review focus 2: create a dangling symlink destination. Assert preflight and burn reject it and its link target bytes remain unchanged.
- [ ] Review focus 3: load a valid snapshot, replace the saved draft with invalid JSON and invoke the private snapshot preflight. Assert source checks still use the original snapshot; retain the public path wrapper's invalid-file rejection.
- [ ] Review focus 4: prepare a Publication, rename its parent and create a replacement parent at the old path. Publish and assert output lands in the original directory inode, with no file in the replacement.
- [ ] Review focus 5: inject a write/validation error followed by unlink failure. Assert original phase/diagnostic survives and cleanup diagnostic/temp path are attached.
- [ ] Source copy faults: deterministic Read adapters return early EOF, appended byte, and same-size changed bytes. Assert rejection. A partial Write adapter exercises write_all behavior; injected disk-write failure triggers transaction cleanup.
- [ ] Publication race: coordinate two prepared writers through a barrier immediately before publication; exactly one no-replace rename succeeds. Verify winning content and losing cleanup. No sleeps.
- [ ] Inject each prepublication phase failure and assert this invocation publishes nothing. Inject postpublication directory-sync failure and assert a published/uncertain result plus a valid final archive.
- [ ] Writer arithmetic: unit-test central/end record serialization at synthetic offsets greater than u32::MAX through a counting sink/private record emitter; assert encoded u64 values and bounds. Exercise checked overflow without multi-gigabyte I/O. Keep existing sparse-parser tests in the focused gate.

Representative assertions after fixture setup:

```rust
let first = burn(&draft_path, directory.path().join("one.vdisc")).unwrap();
let second = burn(&draft_path, directory.path().join("two.vdisc")).unwrap();
assert_ne!(first.disc_id, second.disc_id);
assert_eq!(first.disc_id.get_version_num(), 4);
assert_eq!(validate_vdisc(&first.output_path).unwrap().manifest().disc_id,
           first.disc_id.to_string());
assert!(matches!(first.durability, BurnDurability::Synced));
```

Expected deliverable: integration coverage and deterministic internal failure coverage, ready for the complete gate.

## Task 6 — Verify, review and hand off

- [ ] Run the required sequence, stopping at the first failed command:

```bash
cargo fmt
cargo fmt --check
cargo test -p vdisc-core --lib --test burn --test preflight \
  --test vdisc_archive --test vdisc_schema --test vdisc_limits --test vdisc_workers
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] If this environment still lacks Rust, package the complete replacement/new files plus a stop-on-error verification script for the user's existing repository. State that local checks are pending; inspect the returned full log before declaring completion.
- [ ] Obtain one independent code review focused on publication, cleanup, copied-byte integrity and snapshot consistency. Address concrete findings and rerun affected required gates after changes.
- [ ] Update implementation documentation with actual verification results. Review changed paths and stage only Objective 13 work; exclude logs and generated personal media. Commit after checks pass, using `feat: add synchronous VDISC burner`.

## Plan self-review

Spec coverage: snapshot and source contract → Tasks 1/4; ZIP profile → Task 2; public outcome/error model → Task 3; publication and cleanup → Tasks 3/4; all acceptance cases → Task 5; check ordering and reporting → Task 6.

Types used by later tasks are introduced above. New internal error file is a responsibility split of the spec's burn module. No unrelated refactor or schema change is included. Implementation and tests remain separate phases as requested.

## Execution handoff

Recommended: native execution in this conversation, followed by one independent review. The writer receipts, transaction state and error types depend closely on each other; a single implementer avoids repeated interface handoffs. Delegated per-task implementation remains an option if the user prefers it.

This plan is for review before implementation. No Objective 13 product source has been changed by writing it.
