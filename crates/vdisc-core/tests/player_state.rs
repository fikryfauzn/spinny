#![cfg(target_os = "linux")]

#[path = "support/vdisc_fixture.rs"]
mod fixture;

use serde_json::{Value, json};
use std::{error::Error, fs, path::Path};
use vdisc_core::{Player, PlayerAction, PlayerBoundary, PlayerError, PlayerState};

type TestResult = std::result::Result<(), Box<dyn Error>>;

fn write_disc(path: &Path, tracks: usize, include_duration: bool) -> TestResult {
    let mut manifest = fixture::manifest();
    let template = manifest["tracks"][0].clone();
    let mut track_values = Vec::with_capacity(tracks);
    let mut payloads = Vec::with_capacity(tracks);

    for index in 0..tracks {
        let archive_path = format!("tracks/{:02}.wav", index + 1);
        let mut track = template.clone();
        track["path"] = json!(archive_path);
        track["title"] = json!(format!("Track {}", index + 1));
        if !include_duration {
            track.as_object_mut().unwrap().remove("duration_ms");
        }
        track_values.push(track);
        payloads.push((archive_path, fixture::wav()));
    }

    manifest["tracks"] = Value::Array(track_values);
    fs::write(
        path,
        fixture::build(&fixture::entries(manifest, payloads), false, false),
    )?;
    Ok(())
}

fn inserted_player(tracks: usize) -> Result<(tempfile::TempDir, Player), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("disc.vdisc");
    write_disc(&path, tracks, true)?;
    let mut player = Player::new();
    player.insert(&path)?;
    Ok((dir, player))
}

fn snapshot(player: &Player) -> (PlayerState, Option<usize>, u64, Option<String>) {
    (
        player.state(),
        player.current_track_index(),
        player.position_ms(),
        player.disc().map(|disc| disc.disc_id().to_owned()),
    )
}

fn assert_invalid(error: PlayerError, action: PlayerAction, state: PlayerState) {
    assert!(matches!(
        error,
        PlayerError::InvalidTransition {
            action: actual_action,
            state: actual_state,
        } if actual_action == action && actual_state == state
    ));
}

#[test]
fn new_player_is_empty_and_empty_commands_fail_without_mutation() -> TestResult {
    let mut player = Player::new();
    assert_eq!(player.state(), PlayerState::Empty);
    assert!(player.disc().is_none());
    assert_eq!(player.track_count(), 0);
    assert_eq!(player.current_track_index(), None);
    assert!(player.current_track().is_none());
    assert_eq!(player.position_ms(), 0);

    for (action, result) in [
        (PlayerAction::Eject, player.eject()),
        (PlayerAction::Play, player.play()),
        (PlayerAction::Pause, player.pause()),
        (PlayerAction::Stop, player.stop()),
        (PlayerAction::Next, player.next_track()),
        (PlayerAction::Previous, player.previous()),
        (PlayerAction::Seek, player.seek(1)),
        (PlayerAction::TrackFinished, player.track_finished()),
    ] {
        assert_invalid(result.unwrap_err(), action, PlayerState::Empty);
        assert_eq!(snapshot(&player), (PlayerState::Empty, None, 0, None));
    }

    Ok(())
}

#[test]
fn valid_insert_enters_stopped_on_first_track_and_second_insert_is_rejected() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = dir.path().join("first.vdisc");
    let second = dir.path().join("second.vdisc");
    write_disc(&first, 3, true)?;
    write_disc(&second, 2, true)?;

    let mut player = Player::new();
    player.insert(&first)?;
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.track_count(), 3);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(
        player.current_track().unwrap().title.as_deref(),
        Some("Track 1")
    );
    assert_eq!(player.position_ms(), 0);

    let before = snapshot(&player);
    assert_invalid(
        player.insert(&second).unwrap_err(),
        PlayerAction::Insert,
        PlayerState::Stopped,
    );
    assert_eq!(snapshot(&player), before);
    Ok(())
}

#[test]
fn invalid_disc_never_enters_a_playable_state() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("bad.vdisc");
    fs::write(&path, b"not a VDISC")?;

    let mut player = Player::new();
    assert!(matches!(player.insert(&path), Err(PlayerError::Disc(_))));
    assert_eq!(snapshot(&player), (PlayerState::Empty, None, 0, None));

    write_disc(&path, 1, true)?;
    player.insert(&path)?;
    assert_eq!(player.state(), PlayerState::Stopped);
    Ok(())
}

#[test]
fn play_pause_and_resume_have_explicit_transitions() -> TestResult {
    let (_dir, mut player) = inserted_player(2)?;

    assert_invalid(
        player.pause().unwrap_err(),
        PlayerAction::Pause,
        PlayerState::Stopped,
    );

    player.play()?;
    assert_eq!(player.state(), PlayerState::Playing);
    assert_invalid(
        player.play().unwrap_err(),
        PlayerAction::Play,
        PlayerState::Playing,
    );

    player.seek(5)?;
    player.pause()?;
    assert_eq!(player.state(), PlayerState::Paused);
    assert_eq!(player.position_ms(), 5);
    assert_invalid(
        player.pause().unwrap_err(),
        PlayerAction::Pause,
        PlayerState::Paused,
    );

    player.play()?;
    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(player.position_ms(), 5);
    Ok(())
}

#[test]
fn stop_rewinds_selected_track_and_is_idempotent() -> TestResult {
    let (_dir, mut player) = inserted_player(3)?;
    player.next_track()?;
    player.play()?;
    player.seek(7)?;
    player.stop()?;

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(1));
    assert_eq!(player.position_ms(), 0);

    player.stop()?;
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(1));

    player.play()?;
    player.pause()?;
    player.seek(4)?;
    player.stop()?;
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(1));
    assert_eq!(player.position_ms(), 0);
    Ok(())
}

#[test]
fn eject_from_every_loaded_state_returns_to_empty() -> TestResult {
    for target in [
        PlayerState::Stopped,
        PlayerState::Playing,
        PlayerState::Paused,
    ] {
        let (_dir, mut player) = inserted_player(2)?;
        if target == PlayerState::Playing {
            player.play()?;
        } else if target == PlayerState::Paused {
            player.play()?;
            player.pause()?;
        }

        player.eject()?;
        assert_eq!(snapshot(&player), (PlayerState::Empty, None, 0, None));
    }
    Ok(())
}

#[test]
fn next_preserves_transport_state_resets_position_and_never_wraps() -> TestResult {
    for target in [
        PlayerState::Stopped,
        PlayerState::Playing,
        PlayerState::Paused,
    ] {
        let (_dir, mut player) = inserted_player(3)?;
        if target == PlayerState::Playing {
            player.play()?;
            player.seek(4)?;
        } else if target == PlayerState::Paused {
            player.play()?;
            player.seek(4)?;
            player.pause()?;
        }

        player.next_track()?;
        assert_eq!(player.state(), target);
        assert_eq!(player.current_track_index(), Some(1));
        assert_eq!(player.position_ms(), 0);
        player.next_track()?;
        assert_eq!(player.current_track_index(), Some(2));

        let before = snapshot(&player);
        assert!(matches!(
            player.next_track(),
            Err(PlayerError::Boundary {
                action: PlayerAction::Next,
                boundary: PlayerBoundary::EndOfDisc,
            })
        ));
        assert_eq!(snapshot(&player), before);
    }
    Ok(())
}

#[test]
fn previous_preserves_transport_state_resets_position_and_never_wraps() -> TestResult {
    for target in [
        PlayerState::Stopped,
        PlayerState::Playing,
        PlayerState::Paused,
    ] {
        let (_dir, mut player) = inserted_player(3)?;
        player.next_track()?;
        player.next_track()?;
        if target == PlayerState::Playing {
            player.play()?;
            player.seek(4)?;
        } else if target == PlayerState::Paused {
            player.play()?;
            player.seek(4)?;
            player.pause()?;
        }

        player.previous()?;
        assert_eq!(player.state(), target);
        assert_eq!(player.current_track_index(), Some(1));
        assert_eq!(player.position_ms(), 0);
        player.previous()?;
        assert_eq!(player.current_track_index(), Some(0));

        let before = snapshot(&player);
        assert!(matches!(
            player.previous(),
            Err(PlayerError::Boundary {
                action: PlayerAction::Previous,
                boundary: PlayerBoundary::StartOfDisc,
            })
        ));
        assert_eq!(snapshot(&player), before);
    }
    Ok(())
}

#[test]
fn seek_is_only_for_playing_or_paused_and_known_duration_is_enforced() -> TestResult {
    let (_dir, mut player) = inserted_player(1)?;
    assert_invalid(
        player.seek(1).unwrap_err(),
        PlayerAction::Seek,
        PlayerState::Stopped,
    );

    player.play()?;
    player.seek(10)?;
    assert_eq!(player.position_ms(), 10);
    let before = snapshot(&player);
    assert!(matches!(
        player.seek(11),
        Err(PlayerError::SeekOutOfRange {
            requested_ms: 11,
            duration_ms: 10,
        })
    ));
    assert_eq!(snapshot(&player), before);

    player.pause()?;
    player.seek(3)?;
    assert_eq!(player.state(), PlayerState::Paused);
    assert_eq!(player.position_ms(), 3);
    Ok(())
}

#[test]
fn seek_without_declared_duration_keeps_state_layer_from_inventing_a_limit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("unknown-duration.vdisc");
    write_disc(&path, 1, false)?;

    let mut player = Player::new();
    player.insert(&path)?;
    player.play()?;
    player.seek(60_000)?;
    assert_eq!(player.position_ms(), 60_000);
    Ok(())
}

#[test]
fn track_finished_advances_while_playing_and_final_track_stops_at_disc_start() -> TestResult {
    let (_dir, mut player) = inserted_player(2)?;
    assert_invalid(
        player.track_finished().unwrap_err(),
        PlayerAction::TrackFinished,
        PlayerState::Stopped,
    );

    player.play()?;
    player.track_finished()?;
    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(player.current_track_index(), Some(1));
    assert_eq!(player.position_ms(), 0);

    player.track_finished()?;
    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(player.position_ms(), 0);

    player.play()?;
    player.pause()?;
    let before = snapshot(&player);
    assert_invalid(
        player.track_finished().unwrap_err(),
        PlayerAction::TrackFinished,
        PlayerState::Paused,
    );
    assert_eq!(snapshot(&player), before);
    Ok(())
}
