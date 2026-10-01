#![cfg(target_os = "linux")]

#[path = "support/vdisc_fixture.rs"]
mod fixture;

use std::{
    collections::VecDeque,
    fs,
    sync::{Arc, Mutex},
};

use vdisc_core::{
    BurnedDisc, PlaybackBackend, PlaybackBackendEvent, PlaybackError, PlaybackPlayer,
    PlaybackResult, PlaybackSession, PlayerBoundary, PlayerError, PlayerState,
};

#[derive(Debug, Default)]
struct Shared {
    events: VecDeque<PlaybackBackendEvent>,
    opened: Vec<(usize, u64)>,
    plays: usize,
    pauses: usize,
    fail_open: bool,
    position_ms: u64,
}

#[derive(Debug, Clone)]
struct FakeBackend {
    shared: Arc<Mutex<Shared>>,
}

impl FakeBackend {
    fn new() -> (Self, Arc<Mutex<Shared>>) {
        let shared = Arc::new(Mutex::new(Shared::default()));
        (
            Self {
                shared: Arc::clone(&shared),
            },
            shared,
        )
    }
}

#[derive(Debug)]
struct FakeSession {
    shared: Arc<Mutex<Shared>>,
    base_position_ms: u64,
}

impl PlaybackSession for FakeSession {
    fn play(&self) -> PlaybackResult<()> {
        self.shared.lock().unwrap().plays += 1;
        Ok(())
    }

    fn pause(&self) -> PlaybackResult<()> {
        self.shared.lock().unwrap().pauses += 1;
        Ok(())
    }

    fn position_ms(&self) -> u64 {
        let shared = self.shared.lock().unwrap();
        self.base_position_ms.saturating_add(shared.position_ms)
    }

    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        self.shared.lock().unwrap().events.pop_front()
    }
}

impl PlaybackBackend for FakeBackend {
    type Session = FakeSession;

    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track_index: usize,
        position_ms: u64,
    ) -> PlaybackResult<Self::Session> {
        let mut shared = self.shared.lock().unwrap();
        if shared.fail_open {
            return Err(PlaybackError::Device("synthetic open failure".into()));
        }
        assert!(track_index < disc.track_count());
        shared.opened.push((track_index, position_ms));
        drop(shared);

        Ok(FakeSession {
            shared: Arc::clone(&self.shared),
            base_position_ms: position_ms,
        })
    }
}

fn write_disc(track_count: usize) -> tempfile::TempPath {
    let mut manifest = fixture::manifest();
    let mut tracks = Vec::new();
    let mut payloads = Vec::new();

    for index in 0..track_count {
        let path = format!("tracks/{:02}.wav", index + 1);
        let mut track = manifest["tracks"][0].clone();
        track["path"] = serde_json::json!(path);
        track["duration_ms"] = serde_json::json!(1000u64);
        tracks.push(track);
        payloads.push((path, fixture::wav()));
    }

    manifest["tracks"] = serde_json::json!(tracks);
    let bytes = fixture::build(&fixture::entries(manifest, payloads), false, false);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), bytes).unwrap();
    file.into_temp_path()
}

fn inserted(
    track_count: usize,
) -> (
    PlaybackPlayer<FakeBackend>,
    Arc<Mutex<Shared>>,
    tempfile::TempPath,
) {
    let disc = write_disc(track_count);
    let (backend, shared) = FakeBackend::new();
    let mut player = PlaybackPlayer::with_backend(backend);
    player.insert(&disc).unwrap();
    (player, shared, disc)
}

#[test]
fn track_one_starts_from_zero() {
    let (mut player, shared, _disc) = inserted(3);
    player.play().unwrap();

    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(shared.lock().unwrap().opened, vec![(0, 0)]);
    assert_eq!(shared.lock().unwrap().plays, 1);
}

#[test]
fn pause_and_resume_control_backend_and_state() {
    let (mut player, shared, _disc) = inserted(2);
    player.play().unwrap();
    player.pause().unwrap();

    assert_eq!(player.state(), PlayerState::Paused);
    assert_eq!(shared.lock().unwrap().pauses, 1);

    player.play().unwrap();
    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(shared.lock().unwrap().plays, 2);
}

#[test]
fn next_and_previous_restart_selected_payload_without_wrapping() {
    let (mut player, shared, _disc) = inserted(3);
    player.play().unwrap();

    player.next_track().unwrap();
    assert_eq!(player.current_track_index(), Some(1));
    player.previous().unwrap();
    assert_eq!(player.current_track_index(), Some(0));

    let error = player.previous().unwrap_err();
    assert!(matches!(
        error,
        PlaybackError::Player(PlayerError::Boundary {
            boundary: PlayerBoundary::StartOfDisc,
            ..
        })
    ));

    assert_eq!(shared.lock().unwrap().opened, vec![(0, 0), (1, 0), (0, 0)]);
}

#[test]
fn seek_reopens_same_track_at_requested_position() {
    let (mut player, shared, _disc) = inserted(2);
    player.play().unwrap();

    player.seek(400).unwrap();

    assert_eq!(player.position_ms(), 400);
    assert_eq!(shared.lock().unwrap().opened, vec![(0, 0), (0, 400)]);
}

#[test]
fn backend_eof_advances_then_final_eof_stops_at_disc_start() {
    let (mut player, shared, _disc) = inserted(2);
    player.play().unwrap();

    shared
        .lock()
        .unwrap()
        .events
        .push_back(PlaybackBackendEvent::TrackFinished);
    player.poll().unwrap();

    assert_eq!(player.state(), PlayerState::Playing);
    assert_eq!(player.current_track_index(), Some(1));

    shared
        .lock()
        .unwrap()
        .events
        .push_back(PlaybackBackendEvent::TrackFinished);
    player.poll().unwrap();

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.current_track_index(), Some(0));
    assert_eq!(player.position_ms(), 0);
}

#[test]
fn stop_drops_active_session_and_rewinds() {
    let (mut player, _shared, _disc) = inserted(2);
    player.play().unwrap();
    player.stop().unwrap();

    assert_eq!(player.state(), PlayerState::Stopped);
    assert_eq!(player.position_ms(), 0);
}

#[test]
fn eject_while_playing_returns_to_empty() {
    let (mut player, _shared, _disc) = inserted(2);
    player.play().unwrap();
    player.eject().unwrap();

    assert_eq!(player.state(), PlayerState::Empty);
    assert_eq!(player.current_track_index(), None);
}

#[test]
fn device_error_is_controlled_and_stops_transport() {
    let (mut player, shared, _disc) = inserted(2);
    player.play().unwrap();

    shared
        .lock()
        .unwrap()
        .events
        .push_back(PlaybackBackendEvent::DeviceError("gone".into()));

    let error = player.poll().unwrap_err();
    assert!(matches!(error, PlaybackError::Device(message) if message == "gone"));
    assert_eq!(player.state(), PlayerState::Stopped);
}

#[test]
fn late_decode_error_is_controlled_and_stops_transport() {
    let (mut player, shared, _disc) = inserted(2);
    player.play().unwrap();

    shared
        .lock()
        .unwrap()
        .events
        .push_back(PlaybackBackendEvent::DecodeError("bad frame".into()));

    let error = player.poll().unwrap_err();
    assert!(matches!(error, PlaybackError::Decode(message) if message == "bad frame"));
    assert_eq!(player.state(), PlayerState::Stopped);
}

#[test]
fn output_open_failure_does_not_claim_playing_state() {
    let disc = write_disc(1);
    let (backend, shared) = FakeBackend::new();
    shared.lock().unwrap().fail_open = true;

    let mut player = PlaybackPlayer::with_backend(backend);
    player.insert(&disc).unwrap();

    assert!(player.play().is_err());
    assert_eq!(player.state(), PlayerState::Stopped);
}
