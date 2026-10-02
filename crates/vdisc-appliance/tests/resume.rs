use vdisc_appliance::{
    De200Controller, DiscState, LidState, PlaybackPosition, ResumePlayError, ResumePlaybackPort,
    TransportState, TransportTransitionErrorKind, Volume,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendError {
    Seek,
    Play,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendCall {
    Seek(PlaybackPosition),
    Play,
}

#[derive(Debug, Default)]
struct MockPort {
    calls: Vec<BackendCall>,
    fail_seek: bool,
    fail_play: bool,
}

impl ResumePlaybackPort for MockPort {
    type Error = BackendError;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Seek(position));
        if self.fail_seek {
            return Err(BackendError::Seek);
        }
        Ok(())
    }

    fn play(&mut self) -> Result<(), Self::Error> {
        self.calls.push(BackendCall::Play);
        if self.fail_play {
            return Err(BackendError::Play);
        }
        Ok(())
    }
}

fn controller() -> De200Controller {
    De200Controller::new(Volume::new(0.5).expect("test volume should be valid"))
}

fn seat_and_close_disc(controller: &mut De200Controller) {
    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_insert().unwrap();
    controller.notify_disc_validation_accepted().unwrap();
    controller.notify_disc_seated().unwrap();
    controller.request_lid_close().unwrap();
    controller.notify_lid_closed().unwrap();

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
}

fn stop_at(controller: &mut De200Controller, position: PlaybackPosition) {
    controller.request_play().unwrap();
    controller.request_stop(position).unwrap();
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(position));
}

#[test]
fn first_play_without_resume_calls_only_play() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let mut port = MockPort::default();

    controller.request_play_with(&mut port).unwrap();

    assert_eq!(port.calls, vec![BackendCall::Play]);
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(controller.resume_position(), None);
}

#[test]
fn play_after_stop_executes_seek_then_play_in_order() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(91_250);
    stop_at(&mut controller, remembered);
    let mut port = MockPort::default();

    controller.request_play_with(&mut port).unwrap();

    assert_eq!(
        port.calls,
        vec![BackendCall::Seek(remembered), BackendCall::Play]
    );
    assert_eq!(controller.transport_state(), TransportState::Playing);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn plain_play_cannot_bypass_pending_resume_memory() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(8_000);
    stop_at(&mut controller, remembered);

    let error = controller.request_play().unwrap_err();

    assert_eq!(
        error.kind(),
        TransportTransitionErrorKind::ResumeExecutionRequired
    );
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn seek_failure_leaves_controller_stopped_and_does_not_attempt_play() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(17_000);
    stop_at(&mut controller, remembered);
    let mut port = MockPort {
        fail_seek: true,
        ..MockPort::default()
    };

    let error = controller.request_play_with(&mut port).unwrap_err();

    assert_eq!(error, ResumePlayError::Backend(BackendError::Seek));
    assert_eq!(port.calls, vec![BackendCall::Seek(remembered)]);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn play_failure_after_successful_seek_leaves_controller_stopped() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(22_000);
    stop_at(&mut controller, remembered);
    let mut port = MockPort {
        fail_play: true,
        ..MockPort::default()
    };

    let error = controller.request_play_with(&mut port).unwrap_err();

    assert_eq!(error, ResumePlayError::Backend(BackendError::Play));
    assert_eq!(
        port.calls,
        vec![BackendCall::Seek(remembered), BackendCall::Play]
    );
    assert_eq!(controller.transport_state(), TransportState::Stopped);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn pause_does_not_clear_resume_memory_after_resumed_play() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(31_000);
    stop_at(&mut controller, remembered);
    let mut port = MockPort::default();
    controller.request_play_with(&mut port).unwrap();

    controller.request_pause().unwrap();

    assert_eq!(controller.transport_state(), TransportState::Paused);
    assert_eq!(controller.resume_position(), Some(remembered));
}

#[test]
fn completed_disc_removal_clears_resume_memory_for_the_loaded_session() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(44_000);
    stop_at(&mut controller, remembered);

    controller.request_lid_open().unwrap();
    controller.notify_lid_opened().unwrap();
    controller.request_disc_remove().unwrap();

    assert_eq!(controller.resume_position(), Some(remembered));

    controller.notify_disc_removed().unwrap();

    assert_eq!(controller.disc_state(), DiscState::Absent);
    assert_eq!(controller.resume_position(), None);
}

#[test]
fn failed_resume_play_preserves_mechanical_state() {
    let mut controller = controller();
    seat_and_close_disc(&mut controller);
    let remembered = PlaybackPosition::from_millis(55_000);
    stop_at(&mut controller, remembered);
    let mut port = MockPort {
        fail_play: true,
        ..MockPort::default()
    };

    let _ = controller.request_play_with(&mut port).unwrap_err();

    assert_eq!(controller.lid_state(), LidState::Closed);
    assert_eq!(controller.disc_state(), DiscState::Seated);
    assert_eq!(controller.transport_state(), TransportState::Stopped);
}
