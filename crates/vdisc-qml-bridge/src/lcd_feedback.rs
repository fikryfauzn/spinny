use std::{
    error::Error,
    time::{Duration, Instant},
};
use vdisc_appliance::{
    AvlsTransitionError, AvlsTransitionErrorKind, LcdTransientMessage, LidTransitionError,
    NavigationTransitionError, NavigationTransitionErrorKind, PlayModeTransitionError,
    PlayModeTransitionErrorKind, TransportTransitionError, TransportTransitionErrorKind,
};

#[derive(Default)]
pub(crate) struct LcdFeedback {
    deadline: Option<Instant>,
}
impl LcdFeedback {
    pub(crate) fn record_rejection(&mut self, error: &(dyn Error + 'static), now: Instant) {
        let mut current = Some(error);
        while let Some(error) = current {
            let hold = error
                .downcast_ref::<LidTransitionError>()
                .is_some_and(|e| e.hold_enabled())
                || error
                    .downcast_ref::<TransportTransitionError>()
                    .is_some_and(|e| e.kind() == TransportTransitionErrorKind::HoldEnabled)
                || error
                    .downcast_ref::<NavigationTransitionError>()
                    .is_some_and(|e| e.kind() == NavigationTransitionErrorKind::HoldEnabled)
                || error
                    .downcast_ref::<PlayModeTransitionError>()
                    .is_some_and(|e| e.kind() == PlayModeTransitionErrorKind::HoldEnabled)
                || error
                    .downcast_ref::<AvlsTransitionError>()
                    .is_some_and(|e| e.kind() == AvlsTransitionErrorKind::HoldEnabled);
            if hold {
                self.deadline = now.checked_add(Duration::from_millis(1500));
                return;
            }
            current = error.source();
        }
    }
    pub(crate) fn clear(&mut self) {
        self.deadline = None;
    }
    pub(crate) fn expire(&mut self, now: Instant) {
        if !self.active(now) {
            self.clear();
        }
    }
    pub(crate) fn active(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|deadline| now < deadline)
    }
    pub(crate) fn transient(&self, now: Instant) -> Option<LcdTransientMessage> {
        self.active(now).then_some(LcdTransientMessage::Hold)
    }
}

#[cfg(test)]
#[path = "../../vdisc-appliance/tests/support/audio_backend.rs"]
mod audio;

#[cfg(test)]
mod tests {
    use super::audio::AudioBackend;
    use crate::{ApplianceRuntime, Command};
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };

    fn later(now: Instant, ms: u64) -> Instant {
        now + Duration::from_millis(ms)
    }
    fn runtime() -> ApplianceRuntime<AudioBackend> {
        ApplianceRuntime::with_backend(AudioBackend::default())
    }
    fn playing(backend: AudioBackend, now: Instant) -> ApplianceRuntime<AudioBackend> {
        let mut r = ApplianceRuntime::with_backend(backend);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/vdisc/transport-two-track.vdisc");
        for command in [
            Command::Open,
            Command::LidOpened,
            Command::Insert(path),
            Command::DiscInserted,
            Command::Close,
            Command::LidClosed,
            Command::Play,
        ] {
            r.execute_at(command, now).unwrap();
        }
        r
    }
    #[test]
    fn stopped_hold_expires_at_exact_deadline_without_audio_poll() {
        let now = Instant::now();
        let mut r = runtime();
        r.execute_at(Command::SetHold(true), now).unwrap();
        assert!(!r.snapshot_at(now).lcd_feedback_active);
        assert!(r.execute_at(Command::MenuShort, now).is_err());
        assert_eq!(r.snapshot_at(later(now, 1499)).lcd_message, "Hold");
        assert!(r.snapshot_at(later(now, 1499)).lcd_feedback_active);
        assert_eq!(r.snapshot_at(later(now, 1500)).lcd_message, "");
        assert!(!r.snapshot_at(later(now, 1500)).lcd_feedback_active);
        assert_eq!(r.snapshot_at(later(now, 3000)).transport_state, "Stopped");
    }
    #[test]
    fn repeated_rejection_restarts_but_volume_does_not_extend_deadline() {
        let now = Instant::now();
        let mut r = runtime();
        r.execute_at(Command::SetHold(true), now).unwrap();
        assert!(r.execute_at(Command::ToggleAvls, now).is_err());
        r.execute_at(Command::SetVolume(0.3), later(now, 1000))
            .unwrap();
        assert!(!r.snapshot_at(later(now, 1500)).lcd_feedback_active);
        assert!(r.execute_at(Command::MenuLong, later(now, 1000)).is_err());
        assert!(r.snapshot_at(later(now, 2499)).lcd_feedback_active);
        assert!(!r.snapshot_at(later(now, 2500)).lcd_feedback_active);
        r.execute_at(Command::SetHold(false), later(now, 1100))
            .unwrap();
        assert_eq!(r.snapshot_at(later(now, 1100)).lcd_message, "");
    }
    #[test]
    fn actual_typed_transport_navigation_and_lid_rejections_show_hold() {
        let now = Instant::now();
        for command in [
            Command::Play,
            Command::Pause,
            Command::Stop,
            Command::Next,
            Command::Previous,
            Command::ScanBegin(vdisc_appliance::ScanDirection::Forward),
            Command::ScanStep(0),
            Command::Open,
            Command::MenuShort,
            Command::MenuLong,
        ] {
            let mut r = playing(AudioBackend::default(), now);
            r.execute_at(Command::SetHold(true), now).unwrap();
            let name = format!("{command:?}");
            assert!(r.execute_at(command, now).is_err());
            assert_eq!(r.snapshot_at(now).lcd_message, "Hold", "{name}");
        }
    }
    #[test]
    fn other_rejections_and_completion_callbacks_do_not_create_hold() {
        let now = Instant::now();
        let mut r = runtime();
        assert!(r.execute_at(Command::Play, now).is_err());
        r.execute_at(Command::SetHold(true), now).unwrap();
        for command in [
            Command::LidClosed,
            Command::DiscInserted,
            Command::ScanEnd,
            Command::ScanStep(-1),
            Command::SetVolume(f32::NAN),
        ] {
            assert!(r.execute_at(command, now).is_err());
            assert_eq!(r.snapshot_at(now).lcd_message, "");
        }
    }
    #[test]
    fn precommand_backend_failure_is_not_a_hold_rejection() {
        let now = Instant::now();
        for event in [
            vdisc_core::PlaybackBackendEvent::DeviceError("gone".into()),
            vdisc_core::PlaybackBackendEvent::DecodeError("HoldEnabled".into()),
        ] {
            let backend = AudioBackend::default();
            let mut r = playing(backend.clone(), now);
            r.execute_at(Command::Pause, now).unwrap();
            r.execute_at(Command::SetHold(true), now).unwrap();
            backend.queue(event);
            assert!(r.execute_at(Command::MenuShort, now).is_err());
            assert!(!r.snapshot_at(now).lcd_feedback_active);
            assert_ne!(r.snapshot_at(now).lcd_message, "Hold");
            assert_eq!(r.snapshot_at(now).transport_state, "Stopped");
        }
    }
    #[test]
    fn machine_error_wins_and_expired_hold_never_revives_after_recovery() {
        let now = Instant::now();
        let backend = AudioBackend::default();
        let mut r = playing(backend.clone(), now);
        r.execute_at(Command::SetHold(true), now).unwrap();
        assert!(r.execute_at(Command::MenuShort, now).is_err());
        backend.queue(vdisc_core::PlaybackBackendEvent::DeviceError("gone".into()));
        assert!(
            r.execute_at(Command::SetVolume(0.2), later(now, 200))
                .is_err()
        );
        let s = r.snapshot_at(later(now, 200));
        assert_eq!(s.lcd_message, "AudioOutputFailure");
        assert_eq!(s.lcd_track_number, -1);
        assert_eq!(s.lcd_total_tracks, -1);
        assert!(s.lcd_hold);
        assert!(s.lcd_feedback_active);
        r.controller.clear_error_state();
        assert_eq!(r.snapshot_at(later(now, 1499)).lcd_message, "Hold");
        assert_eq!(r.snapshot_at(later(now, 1500)).lcd_message, "");
    }
    #[test]
    fn string_lookalike_error_does_not_start_feedback() {
        let mut f = super::LcdFeedback::default();
        let now = Instant::now();
        f.record_rejection(&std::io::Error::other("HoldEnabled"), now);
        assert!(!f.active(now));
    }
}
