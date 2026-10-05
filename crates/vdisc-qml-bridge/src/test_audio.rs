//! Explicit QuickTest-only injection. Neither missing hardware nor platform selects this backend.
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
};
use vdisc_core::{
    BurnedDisc, PlaybackBackend, PlaybackBackendEvent, PlaybackError, PlaybackResult,
    PlaybackSession,
};

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static DRIVER: RefCell<Weak<Mutex<BackendState>>> = const { RefCell::new(Weak::new()) };
}
#[derive(Default)]
struct BackendState {
    current: Weak<Mutex<SessionState>>,
    fail_open: u32,
}
struct SessionState {
    position: u64,
    gain: f32,
    playing: bool,
    events: VecDeque<PlaybackBackendEvent>,
}
pub struct TestBackend(Arc<Mutex<BackendState>>);
pub struct TestSession(Arc<Mutex<SessionState>>);

pub fn create_backend() -> Option<TestBackend> {
    if !ENABLED.get() {
        return None;
    }
    let state = Arc::new(Mutex::new(BackendState::default()));
    DRIVER.with(|driver| *driver.borrow_mut() = Arc::downgrade(&state));
    Some(TestBackend(state))
}
fn current() -> Option<Arc<Mutex<SessionState>>> {
    DRIVER
        .with(|driver| driver.borrow().upgrade())
        .and_then(|state| state.lock().unwrap().current.upgrade())
}
impl PlaybackBackend for TestBackend {
    type Session = TestSession;
    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track: usize,
        position: u64,
    ) -> PlaybackResult<Self::Session> {
        if track >= disc.track_count() {
            return Err(PlaybackError::BackendInvariant("test track outside disc"));
        }
        let mut backend = self.0.lock().unwrap();
        let failure = std::mem::take(&mut backend.fail_open);
        match failure {
            1 => return Err(PlaybackError::NoOutputDevice),
            2 => {
                return Err(PlaybackError::UnsupportedOutput {
                    sample_rate_hz: 44100,
                });
            }
            3 => return Err(PlaybackError::Device("injected device failure".into())),
            4 => return Err(PlaybackError::Decode("injected decode failure".into())),
            _ => {}
        }
        let session = Arc::new(Mutex::new(SessionState {
            position,
            gain: 1.0,
            playing: false,
            events: VecDeque::new(),
        }));
        backend.current = Arc::downgrade(&session);
        Ok(TestSession(session))
    }
}
impl PlaybackSession for TestSession {
    fn play(&self) -> PlaybackResult<()> {
        self.0.lock().unwrap().playing = true;
        Ok(())
    }
    fn pause(&self) -> PlaybackResult<()> {
        self.0.lock().unwrap().playing = false;
        Ok(())
    }
    fn set_gain(&self, gain: f32) -> PlaybackResult<()> {
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return Err(PlaybackError::BackendInvariant("invalid test gain"));
        }
        self.0.lock().unwrap().gain = gain;
        Ok(())
    }
    fn position_ms(&self) -> u64 {
        self.0.lock().unwrap().position
    }
    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        self.0.lock().unwrap().events.pop_front()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vdisc_audio_test_enable() {
    ENABLED.set(true);
}
#[unsafe(no_mangle)]
pub extern "C" fn vdisc_audio_test_set_position(position: u64) {
    if let Some(session) = current() {
        session.lock().unwrap().position = position;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn vdisc_audio_test_queue_event(code: u32) {
    let event = match code {
        1 => PlaybackBackendEvent::TrackFinished,
        2 => PlaybackBackendEvent::DeviceError("injected device failure".into()),
        3 => PlaybackBackendEvent::DecodeError("injected decode failure".into()),
        _ => return,
    };
    if let Some(session) = current() {
        session.lock().unwrap().events.push_back(event);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn vdisc_audio_test_fail_next_open(code: u32) {
    if code > 4 {
        return;
    }
    if let Some(state) = DRIVER.with(|driver| driver.borrow().upgrade()) {
        state.lock().unwrap().fail_open = code;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn vdisc_audio_test_gain() -> f32 {
    current().map_or(-1.0, |session| session.lock().unwrap().gain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_factory_is_opt_in_independent_and_does_not_target_retired_session() {
        ENABLED.set(false);
        assert!(create_backend().is_none());
        vdisc_audio_test_enable();
        let mut first = create_backend().unwrap();
        let disc = BurnedDisc::open(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/vdisc/valid-v1.vdisc"
        ))
        .unwrap();
        let mut old = first.open_session(&disc, 0, 0).unwrap();
        let mut replacement = first.open_session(&disc, 0, 7).unwrap();
        vdisc_audio_test_queue_event(2);
        assert!(old.poll_event().is_none());
        assert!(matches!(
            replacement.poll_event(),
            Some(PlaybackBackendEvent::DeviceError(_))
        ));
        vdisc_audio_test_queue_event(99);
        assert!(replacement.poll_event().is_none());
        let mut second = create_backend().unwrap();
        vdisc_audio_test_fail_next_open(1);
        assert!(matches!(
            second.open_session(&disc, 0, 0),
            Err(PlaybackError::NoOutputDevice)
        ));
        assert!(first.open_session(&disc, 0, 0).is_ok());
        drop(second);
        vdisc_audio_test_queue_event(1);
        assert!(replacement.poll_event().is_none());
        ENABLED.set(false);
    }
}
