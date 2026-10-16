#![cfg(target_os = "linux")]
#[path = "support/vdisc_fixture.rs"]
mod fixture;

use std::fs;
use vdisc_core::{BurnedDisc, Player, PlayerAction, PlayerError, PlayerState};

#[test]
fn verified_disc_is_transferred_without_reopening_path() {
    let dir = tempfile::tempdir().unwrap();
    let disc_path = dir.path().join("original.vdisc");
    let renamed = dir.path().join("renamed.vdisc");
    fs::write(
        &disc_path,
        fixture::build(&fixture::minimal_entries(), false, false),
    )
    .unwrap();
    let verified = BurnedDisc::open(&disc_path).unwrap();
    let id = verified.disc_id().to_owned();
    fs::rename(&disc_path, &renamed).unwrap();
    fs::write(&disc_path, b"not a valid replacement").unwrap();

    let mut player = Player::new();
    player.insert_validated(verified).unwrap();
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(player.disc().unwrap().disc_id(), id);
    player.disc().unwrap().verify_open_file().unwrap();
    assert!(player.disc().unwrap().verify_current_path().is_err());
}

#[test]
fn prevalidated_insert_rejects_second_disc_transactionally() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("disc.vdisc");
    fs::write(
        &path,
        fixture::build(&fixture::minimal_entries(), false, false),
    )
    .unwrap();
    let first = BurnedDisc::open(&path).unwrap();
    let second = BurnedDisc::open(&path).unwrap();
    let mut player = Player::new();
    player.insert_validated(first).unwrap();
    let id = player.disc().unwrap().disc_id().to_owned();
    assert!(matches!(
        player.insert_validated(second),
        Err(PlayerError::InvalidTransition {
            action: PlayerAction::Insert,
            state: PlayerState::Stopped
        })
    ));
    assert_eq!(player.disc().unwrap().disc_id(), id);
    assert_eq!(player.state(), PlayerState::Stopped);
}
