#![cfg(target_os = "linux")]

#[path = "support/vdisc_fixture.rs"]
mod fixture;

use std::{fs, ops::Range, path::Path};

use vdisc_core::{
    format::{FormatErrorKind, validate_vdisc},
    *,
};

fn ready(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let source = dir.join("song.wav");
    fs::write(&source, fixture::wav()).unwrap();

    let draft = dir.join("disc.vdraft");
    save_draft(&draft, &DraftDisc::new("Night Drive").unwrap()).unwrap();

    let selected = AddTrackRequest::begin(&draft)
        .unwrap()
        .select_source(TrackSourceKind::Local)
        .select_local_file(&source)
        .unwrap();

    add_validated_local_track(ValidatedLocalAudio::validate(selected).unwrap()).unwrap();

    (draft, source)
}

fn burn_synced(draft: &Path, output: &Path) {
    let result = burn(draft, output).unwrap();
    assert!(matches!(result.durability, BurnDurability::Synced));
}

// Test-only parser for archives emitted by the Objective 13 writer. This is
// intentionally independent of the production validator and only locates a
// payload byte so the test can simulate external tampering.
fn stored_payload_range(path: &Path, wanted: &str) -> Range<usize> {
    let bytes = fs::read(path).unwrap();
    let u16_at =
        |offset| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
    let u64_at =
        |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;

    let zip64_end = bytes.len() - 98;
    let count = u64_at(zip64_end + 32);
    let mut central = u64_at(zip64_end + 48);

    for _ in 0..count {
        let name_len = u16_at(central + 28);
        let extra_len = u16_at(central + 30);
        let name = std::str::from_utf8(&bytes[central + 46..central + 46 + name_len]).unwrap();

        // The production writer emits the ZIP64 extra field first, containing
        // uncompressed size, compressed size, and local-header offset.
        let zip64_values = central + 46 + name_len + 4;
        let size = u64_at(zip64_values);
        let local = u64_at(zip64_values + 16);
        let data = local + 30 + u16_at(local + 26) + u16_at(local + 28);

        if name == wanted {
            return data..data + size;
        }

        central += 46 + name_len + extra_len;
    }

    panic!("payload {wanted} not found");
}

#[test]
fn burned_disc_is_a_read_only_inspection_domain() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let output = dir.path().join("night-drive.vdisc");

    let burn_result = burn(&draft, &output).unwrap();
    let disc = BurnedDisc::inspect(&output).unwrap();

    assert_eq!(disc.path(), output.as_path());
    let expected_id = burn_result.disc_id.to_string();
    assert_eq!(disc.disc_id(), expected_id.as_str());
    assert_eq!(disc.title(), "Night Drive");
    assert_eq!(disc.track_count(), 1);
    assert_eq!(disc.manifest().disc_id.as_str(), disc.disc_id());
    assert_eq!(disc.integrity().algorithm, "sha256");

    let reopened = BurnedDisc::inspect(&output).unwrap();
    assert_eq!(reopened.disc_id(), disc.disc_id());
}

#[test]
fn draft_persistence_cannot_replace_a_vdisc() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    burn_synced(&draft, &output);

    let before = fs::read(&output).unwrap();
    let replacement = DraftDisc::new("Replacement").unwrap();
    let result = save_draft(&output, &replacement);

    assert!(matches!(
        result,
        Err(VdiscError::DraftTargetNotEditable { .. })
    ));
    assert_eq!(fs::read(&output).unwrap(), before);
    validate_vdisc(&output).unwrap();
}

#[test]
fn burned_bytes_renamed_to_vdraft_still_cannot_be_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    let disguised = dir.path().join("final.vdraft");
    burn_synced(&draft, &output);
    fs::rename(&output, &disguised).unwrap();

    let before = fs::read(&disguised).unwrap();
    let result = save_draft(&disguised, &DraftDisc::new("Replacement").unwrap());

    assert!(matches!(
        result,
        Err(VdiscError::DraftTargetNotEditable { .. })
    ));
    assert_eq!(fs::read(&disguised).unwrap(), before);
    validate_vdisc(&disguised).unwrap();
}

#[test]
fn normal_draft_mutation_entrypoints_reject_a_disguised_burned_disc() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    let disguised = dir.path().join("final.vdraft");
    burn_synced(&draft, &output);
    fs::rename(&output, &disguised).unwrap();

    let before = fs::read(&disguised).unwrap();

    assert!(load_draft(&disguised).is_err());
    assert!(AddTrackRequest::begin(&disguised).is_err());
    assert!(CustomizationSession::begin(&disguised).is_err());
    assert!(remove_draft_track(&disguised, 1).is_err());
    assert!(move_draft_track(&disguised, 1, 1).is_err());

    assert_eq!(fs::read(&disguised).unwrap(), before);
    validate_vdisc(&disguised).unwrap();
}

#[test]
fn existing_unknown_vdraft_is_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("occupied.vdraft");
    fs::write(&path, b"not a draft and not a disc").unwrap();
    let before = fs::read(&path).unwrap();

    let result = save_draft(&path, &DraftDisc::new("Replacement").unwrap());

    assert!(matches!(
        result,
        Err(VdiscError::DraftTargetNotEditable { .. })
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn symlink_vdraft_is_not_a_mutable_draft_destination() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real.vdraft");
    let alias = dir.path().join("alias.vdraft");
    save_draft(&real, &DraftDisc::new("Original").unwrap()).unwrap();
    let before = fs::read(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    let result = save_draft(&alias, &DraftDisc::new("Replacement").unwrap());

    assert!(matches!(
        result,
        Err(VdiscError::DraftTargetNotEditable { .. })
    ));
    assert_eq!(fs::read(&real).unwrap(), before);
    assert_eq!(fs::read_link(&alias).unwrap(), real);
}

#[test]
fn byte_for_byte_copy_preserves_burn_identity_and_validity() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let original = dir.path().join("original.vdisc");
    let copy = dir.path().join("copy.vdisc");
    burn_synced(&draft, &original);

    let original_bytes = fs::read(&original).unwrap();
    fs::copy(&original, &copy).unwrap();

    assert_eq!(fs::read(&copy).unwrap(), original_bytes);
    let a = BurnedDisc::inspect(&original).unwrap();
    let b = BurnedDisc::inspect(&copy).unwrap();
    assert_eq!(a.disc_id(), b.disc_id());
    assert_eq!(a.manifest(), b.manifest());
}

#[test]
fn external_payload_tampering_is_detected_on_reverification() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let output = dir.path().join("final.vdisc");
    burn_synced(&draft, &output);

    let disc = BurnedDisc::inspect(&output).unwrap();
    let range = stored_payload_range(&output, "tracks/01.wav");
    let mut bytes = fs::read(&output).unwrap();
    bytes[range.start] ^= 0x5a;
    fs::write(&output, bytes).unwrap();

    let error = disc.verify_current_path().unwrap_err();
    assert_eq!(error.kind, FormatErrorKind::IntegrityMismatch);
}

#[test]
fn editing_the_draft_and_burning_again_never_mutates_the_first_disc() {
    let dir = tempfile::tempdir().unwrap();
    let (draft, _) = ready(dir.path());
    let first_path = dir.path().join("first.vdisc");
    let second_path = dir.path().join("second.vdisc");

    let first = burn(&draft, &first_path).unwrap();
    let first_bytes = fs::read(&first_path).unwrap();

    CustomizationSession::begin(&draft)
        .unwrap()
        .set_disc_label("Second pressing")
        .unwrap();

    let second = burn(&draft, &second_path).unwrap();

    assert_ne!(first.disc_id, second.disc_id);
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);

    let first_disc = BurnedDisc::inspect(&first_path).unwrap();
    let second_disc = BurnedDisc::inspect(&second_path).unwrap();
    assert!(first_disc.manifest().appearance.label.is_none());
    assert_eq!(
        second_disc.manifest().appearance.label.as_deref(),
        Some("Second pressing")
    );
}
