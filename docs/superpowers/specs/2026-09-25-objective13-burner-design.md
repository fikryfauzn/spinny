# Objective 13 — synchronous VDISC burner

Status: written design for review; no Objective 13 implementation yet.
Baseline: Objective 12 committed locally as `dbc5838`; user verification passed formatting, 89 focused tests, 171 workspace tests and Clippy.
Authority: the approved VDISC V1 format contract and the synchronous/failure/durability decisions accepted in this conversation.

## 1. Outcome and scope

Turn a saved valid draft into a complete, self-contained `.vdisc`, synchronously. The function returns after publishing the file or reporting a failure. It never modifies the draft, source audio or source artwork.

Run preflight internally, assign a fresh UUIDv4 and Unix-seconds timestamp, stream-copy and hash original payload bytes, validate the completed artifact, and publish atomically without replacement. The same draft burned twice to different destinations produces different disc IDs while preserving payload bytes and ordering.

Progress callbacks, cancellation, background job management, CLI command design, playback, domain immutability and the supported public reader are outside this increment. The draft and disc schemas stay at their current versions; no migration is required. Existing helper binaries remain required.

## 2. Public result and error semantics

Expose a synchronous `burn(draft_path, output_path)` function in `vdisc-core`. Both arguments accept `impl AsRef<Path>`.

Use an explicit `Result<BurnResult, BurnError>` to distinguish burn failures from existing draft-edit errors. This refines the earlier illustrative `Result<BurnResult>` spelling without changing its behavior.

`BurnResult` contains:

- `disc_id`: the newly assigned UUID.
- `burned_at_unix`: timestamp recorded in the manifest.
- `output_path`: resolved absolute output path used for the operation.
- `track_count`: number of manifest tracks.
- `size_bytes`: final archive length.
- `durability`: either `Synced` or `PublishedButDirectorySyncFailed`, retaining the underlying sync diagnostic in the latter case.

The final directory-sync failure returns a published result with uncertain durability, not an ordinary failed burn. Callers must inspect durability. They must not automatically retry a published result.

A `BurnError` identifies its phase, underlying diagnostic and relevant source/output path. Preflight rejection retains the complete report. Other categories cover loading, clock failure, source access/change, archive writing, temporary verification, file sync and publication. Cleanup failure must preserve the original failure and expose the leftover temporary path and cleanup diagnostic.

A returned prepublication error means this invocation did not publish the destination. A competing invocation may have created that path independently. Process crashes can prevent any return value; absence of a returned success is not proof that publication never occurred.

## 3. Snapshot and source contract

Load the draft once. Refactor preflight into a shared internal routine taking that loaded snapshot; the public `run_preflight` wrapper preserves its existing API and behavior. The burner uses the same snapshot for validation, manifest projection and payload selection.

Edits made to the saved draft after it is loaded belong to a later burn. They do not silently change this operation. The burner never saves or migrates the draft on disk.

Open each source as a regular file and keep the handle for its copy. The streaming loop counts bytes and computes SHA-256 and CRC32 over exactly the bytes successfully written. Compare copied length and SHA-256 against the snapshot's imported fingerprint. Apply this to artwork as well as every track, including intentional duplicate source selections.

Read the expected byte count plus an EOF check so growing sources cannot extend copying indefinitely. Reject truncation, an extra byte or a hash mismatch. A changed source that yields exactly the expected bytes is acceptable: the contract concerns embedded bytes, not an unverifiable history of filesystem edits. Do not rely on timestamps or preflight hashes alone.

Keep memory bounded using a fixed copy buffer. Never allocate an entire audio entry. Preserve original encoded bytes; no transcoding or tag rewriting.

## 4. Archive writer

Add a private production writer separate from the test fixture generator. Reuse Objective 12 models and invariant checks, and its CRC implementation through a narrow internal interface.

Choose the deliberately narrow Stored ZIP profile already accepted by the validator. The writer emits ZIP64 entries/end records, including for small discs, with signed 64-bit data descriptors. This keeps a single streaming layout and avoids guessing whether a later offset will exceed classic ZIP limits. Small ZIP64 discs are already part of the accepted V1 contract.

Physical order is manifest, ordered tracks, optional artwork, then integrity. Manifest array order remains authoritative for playback.

Construct the manifest using `Manifest::from_draft` and the new identity/time. Serialize bounded JSON and preserve those exact bytes. Record manifest and payload sizes/hashes in integrity; exclude integrity itself. Compute ZIP CRC32 for every entry, including integrity. Entry names, flags, versions, headers and extra fields must match the strict V1 profile. No directory entries, comments or unrelated ZIP extras.

Use checked u64 offset and length arithmetic. Retain at most nine entry records in memory. Arithmetic overflow or unsupported filesystem capacity produces an explicit error. No new product-wide audio size or duration cap.

## 5. Temporary file and publication

Resolve and open the destination parent directory before writing. Use an exclusive, unpredictable temporary filename in that directory, with a recognizable VDISC temporary prefix and restrictive permissions. Pin directory operations to the opened parent on Linux so a renamed/replaced parent path cannot redirect publication.

Do not create missing destination parents. Reject invalid output extensions and any existing destination, including directories and dangling symlinks. The preflight existence check is advisory; the final operation must enforce no replacement atomically.

Sequence:

1. Resolve inputs, load the draft and run snapshot preflight.
2. Assign fresh identity and time, rejecting an invalid system time.
3. Exclusively create the temporary file in the pinned output directory.
4. Write manifest/payloads/integrity and finish ZIP64 structures.
5. Flush buffered output and sync the completed file.
6. Validate the complete temporary artifact using Objective 12 through a path anchored to the pinned directory. Compare the validated manifest/identity to the intended snapshot projection.
7. Publish using Linux atomic no-replace rename relative to the pinned directory. If the filesystem does not support the required guarantee, fail explicitly; never fall back to a replacing rename or check-then-rename.
8. Sync the parent directory. Report `Synced` or a published result with uncertain directory durability.

The rename is the publication point. No fallible operation after that point may be reported as though publication did not happen. Prepare normal result data before publication.

Atomic publication prevents a partial final disc from appearing. Syncing provides the operating system's durability guarantee, subject to filesystem and hardware behavior. It cannot promise protection from every storage failure.

Concurrent burns to the same absent name: at most one succeeds in publishing. Others fail without replacing that file. Burns to distinct names may proceed independently.

## 6. Cleanup and isolation

Before publication, ordinary error paths attempt to unlink only this operation's temporary file. If cleanup fails, preserve both the original failure and the leftover path. Successful atomic rename removes the temporary name.

A process crash or power loss may leave a recognizable temporary file. Do not sweep temporary files automatically in this objective: another burn might still own one.

The trusted local output directory and the application process are not an adversarial security boundary against another process running as the same user. Resource-limited media/image workers contain decoder resource exhaustion; they are not a syscall sandbox.

## 7. Expected file changes

Paths are relative to the repository root. Final extraction of small internal helpers may refine this map in the implementation plan.

| Action | Path | Responsibility |
| --- | --- | --- |
| Create | `crates/vdisc-core/src/burn.rs` | Public API, result/error types and transaction orchestration |
| Create | `crates/vdisc-core/src/burn/publication.rs` | Linux directory anchoring, temporary lifecycle, no-replace publication and syncing |
| Create | `crates/vdisc-core/src/format/writer.rs` | Private streaming Stored ZIP64 writer and entry receipts |
| Modify | `crates/vdisc-core/src/format/mod.rs` | Internal writer registration |
| Modify | `crates/vdisc-core/src/format/zip.rs` | Narrow internal CRC sharing if needed |
| Modify | `crates/vdisc-core/src/preflight.rs` | Shared snapshot validation; retain public path wrapper |
| Modify | `crates/vdisc-core/src/lib.rs` | Export burn API and result/error types |
| Create | `crates/vdisc-core/tests/burn.rs` | Public burn integration tests |
| Modify | `crates/vdisc-core/tests/preflight.rs` | Snapshot-refactor regression coverage where needed |
| Create | `docs/OBJECTIVE_13_IMPLEMENTATION.md` | Usage, guarantees, recovery and verification record |

Dedicated burn errors avoid widening the existing `VdiscError` unless implementation reveals a concrete integration need. Use the existing Linux libc dependency. No additional production dependency is planned.

## 8. Acceptance checks, added after implementation

- Burn one track and six tracks; cover mixed supported codecs, duplicate songs and preserved order/metadata.
- Preserve default white appearance and customized RGB, label/subtitle and optional supported artwork.
- Compare embedded bytes with originals and validate every resulting archive with Objective 12.
- Burn twice to different paths: IDs differ, each is UUIDv4, timestamps are valid, payloads match.
- Validate a produced disc after the original draft/media have been removed from a test directory.
- Reject invalid drafts, missing sources, changed fingerprints and invalid destinations; sources and draft remain byte-identical.
- Inject source changes during copying and verify hash/length rejection and cleanup. Exercise shrinking and growing sources.
- Existing file, directory and dangling symlink destinations remain untouched. A deterministic publication race permits exactly one winner.
- Inject write, file-sync, validation and publication failures before the publication point; no partial final disc is published.
- Inject directory-sync failure after publication: return the published/uncertain result and retain a valid final disc.
- Inject cleanup failure: retain the original error and identify the leftover temporary file.
- Exercise ZIP64 offsets beyond 4 GiB through a synthetic counting/seekable sink without giant allocations or full multi-gigabyte test hashing.
- Test internal fault seams deterministically; no new public fault-injection API, racy global environment changes or timing-based race tests.

After implementation and tests are in place, run `cargo fmt`, `cargo fmt --check`, focused burn/preflight/format tests, `cargo test --workspace`, then `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Stop at a failed gate and fix it. Report any unavailable gate rather than counting it as passed.

## 9. Review checkpoint

The user has approved synchronous burning, normal-failure cleanup, manual crash cleanup, and the published-but-durability-uncertain distinction. The typed result representation, always-ZIP64 writer and Linux publication layout above make that behavior concrete for written review. Implementation planning follows acceptance of this document.
