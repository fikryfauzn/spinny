#![allow(dead_code)]
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use vdisc_core::{
    BurnedDisc, PlaybackBackend, PlaybackBackendEvent, PlaybackError, PlaybackResult,
    PlaybackSession,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Open(usize, u64),
    Gain(f32),
    Play,
    Pause,
    Drop,
}
#[derive(Default)]
pub struct SessionState {
    pub position: u64,
    pub gain: f32,
    pub playing: bool,
    pub events: VecDeque<PlaybackBackendEvent>,
}
#[derive(Default)]
pub struct State {
    pub operations: Vec<Operation>,
    pub sessions: Vec<Arc<Mutex<SessionState>>>,
    pub active: usize,
    pub max_active: usize,
    pub fail_open: Option<PlaybackError>,
    pub fail_operation: Option<&'static str>,
    pub event_polls: usize,
}
#[derive(Clone, Default)]
pub struct AudioBackend(pub Arc<Mutex<State>>);
impl AudioBackend {
    pub fn current(&self) -> Arc<Mutex<SessionState>> {
        self.0.lock().unwrap().sessions.last().unwrap().clone()
    }
    pub fn operations(&self) -> Vec<Operation> {
        self.0.lock().unwrap().operations.clone()
    }
    pub fn queue(&self, event: PlaybackBackendEvent) {
        self.current().lock().unwrap().events.push_back(event);
    }
}
pub struct AudioSession {
    state: Arc<Mutex<State>>,
    session: Arc<Mutex<SessionState>>,
}
impl AudioSession {
    fn operation(&self, name: &'static str, operation: Operation) -> PlaybackResult<()> {
        let mut state = self.state.lock().unwrap();
        state.operations.push(operation);
        if state.fail_operation == Some(name) {
            state.fail_operation = None;
            return Err(PlaybackError::Device(format!("injected {name} failure")));
        }
        Ok(())
    }
}
impl PlaybackSession for AudioSession {
    fn play(&self) -> PlaybackResult<()> {
        self.operation("play", Operation::Play)?;
        let mut state = self.state.lock().unwrap();
        let mut session = self.session.lock().unwrap();
        if !session.playing {
            session.playing = true;
            state.active += 1;
            state.max_active = state.max_active.max(state.active);
        }
        Ok(())
    }
    fn pause(&self) -> PlaybackResult<()> {
        self.operation("pause", Operation::Pause)?;
        let mut state = self.state.lock().unwrap();
        let mut session = self.session.lock().unwrap();
        if session.playing {
            session.playing = false;
            state.active -= 1;
        }
        Ok(())
    }
    fn set_gain(&self, gain: f32) -> PlaybackResult<()> {
        if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return Err(PlaybackError::BackendInvariant("invalid gain"));
        }
        self.operation("gain", Operation::Gain(gain))?;
        self.session.lock().unwrap().gain = gain;
        Ok(())
    }
    fn position_ms(&self) -> u64 {
        self.session.lock().unwrap().position
    }
    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        self.state.lock().unwrap().event_polls += 1;
        self.session.lock().unwrap().events.pop_front()
    }
}
impl Drop for AudioSession {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.operations.push(Operation::Drop);
        if self.session.lock().unwrap().playing {
            state.active -= 1;
        }
    }
}
impl PlaybackBackend for AudioBackend {
    type Session = AudioSession;
    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track: usize,
        position: u64,
    ) -> PlaybackResult<Self::Session> {
        assert!(track < disc.track_count());
        let mut state = self.0.lock().unwrap();
        state.operations.push(Operation::Open(track, position));
        if let Some(error) = state.fail_open.take() {
            return Err(error);
        }
        let session = Arc::new(Mutex::new(SessionState {
            position,
            gain: 1.0,
            ..Default::default()
        }));
        state.sessions.push(session.clone());
        Ok(AudioSession {
            state: self.0.clone(),
            session,
        })
    }
}
