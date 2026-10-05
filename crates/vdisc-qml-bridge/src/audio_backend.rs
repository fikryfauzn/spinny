use vdisc_core::{
    BurnedDisc, CpalBackend, PlaybackBackend, PlaybackBackendEvent, PlaybackResult,
    PlaybackSession, linux_playback::CpalSession,
};

pub enum RuntimeBackend {
    Real(CpalBackend),
    #[cfg(feature = "qml-test-support")]
    Test(crate::test_audio::TestBackend),
}
pub enum RuntimeSession {
    Real(CpalSession),
    #[cfg(feature = "qml-test-support")]
    Test(crate::test_audio::TestSession),
}
impl Default for RuntimeBackend {
    fn default() -> Self {
        #[cfg(feature = "qml-test-support")]
        if let Some(backend) = crate::test_audio::create_backend() {
            return Self::Test(backend);
        }
        Self::Real(CpalBackend)
    }
}
impl PlaybackBackend for RuntimeBackend {
    type Session = RuntimeSession;
    fn open_session(
        &mut self,
        disc: &BurnedDisc,
        track: usize,
        position: u64,
    ) -> PlaybackResult<Self::Session> {
        match self {
            Self::Real(b) => b
                .open_session(disc, track, position)
                .map(RuntimeSession::Real),
            #[cfg(feature = "qml-test-support")]
            Self::Test(b) => b
                .open_session(disc, track, position)
                .map(RuntimeSession::Test),
        }
    }
}
impl RuntimeSession {
    fn session(&self) -> &dyn PlaybackSession {
        match self {
            Self::Real(s) => s,
            #[cfg(feature = "qml-test-support")]
            Self::Test(s) => s,
        }
    }
}
impl PlaybackSession for RuntimeSession {
    fn play(&self) -> PlaybackResult<()> {
        self.session().play()
    }
    fn pause(&self) -> PlaybackResult<()> {
        self.session().pause()
    }
    fn set_gain(&self, gain: f32) -> PlaybackResult<()> {
        self.session().set_gain(gain)
    }
    fn position_ms(&self) -> u64 {
        self.session().position_ms()
    }
    fn poll_event(&mut self) -> Option<PlaybackBackendEvent> {
        match self {
            Self::Real(s) => s.poll_event(),
            #[cfg(feature = "qml-test-support")]
            Self::Test(s) => s.poll_event(),
        }
    }
}
