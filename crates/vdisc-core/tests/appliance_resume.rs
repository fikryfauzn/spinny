#![cfg(target_os = "linux")]

use std::path::Path;

use vdisc_core::{Player, PlayerAction, PlayerError, PlayerState};

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/vdisc/valid-v1.vdisc")
}

#[test]
fn stopped_position_restore_is_distinct_from_interactive_seek() {
    let mut player = Player::new();
    player.insert(fixture()).unwrap();

    let seek_error = player.seek(1).unwrap_err();
    assert!(matches!(
        seek_error,
        PlayerError::InvalidTransition {
            action: PlayerAction::Seek,
            state: PlayerState::Stopped,
        }
    ));

    player.restore_stopped_position(1).unwrap();
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.position_ms(), 1);

    player.play().unwrap();
    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(player.position_ms(), 1);
}

#[test]
fn stopped_position_restore_is_rejected_outside_stopped_transport() {
    let mut player = Player::new();
    player.insert(fixture()).unwrap();
    player.play().unwrap();

    let error = player.restore_stopped_position(0).unwrap_err();
    assert!(matches!(
        error,
        PlayerError::InvalidTransition {
            action: PlayerAction::RestoreStoppedPosition,
            state: PlayerState::Playing,
        }
    ));
}

#[test]
fn normal_stop_contract_still_rewinds_before_appliance_restore() {
    let mut player = Player::new();
    player.insert(fixture()).unwrap();
    player.play().unwrap();
    player.seek(1).unwrap();
    player.stop().unwrap();

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.position_ms(), 0);

    player.restore_stopped_position(1).unwrap();
    assert_eq!(player.position_ms(), 1);
}
