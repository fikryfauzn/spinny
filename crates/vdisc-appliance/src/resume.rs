use std::{error::Error, fmt};

use crate::state::{De200Controller, PlaybackPosition, TransportTransitionError};

/// Backend-neutral capability required to preserve D-E200 resume-after-stop.
///
/// The appliance contract requires a stopped transport to restore its saved
/// position before playback begins. Objective 6 intentionally models that
/// contract without depending on `vdisc-core`; Objective 13 will map this port
/// onto the real backend and resolve any capability mismatch explicitly.
pub trait ResumePlaybackPort {
    type Error;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error>;
    fn play(&mut self) -> Result<(), Self::Error>;
}

/// Failure while executing a resume-aware PLAY request.
#[derive(Debug, PartialEq, Eq)]
pub enum ResumePlayError<E> {
    Transition(TransportTransitionError),
    Backend(E),
}

impl<E> From<TransportTransitionError> for ResumePlayError<E> {
    fn from(error: TransportTransitionError) -> Self {
        Self::Transition(error)
    }
}

impl<E: fmt::Display> fmt::Display for ResumePlayError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transition(error) => write!(f, "PLAY request rejected: {error}"),
            Self::Backend(error) => write!(f, "resume playback backend failed: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for ResumePlayError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transition(error) => Some(error),
            Self::Backend(error) => Some(error),
        }
    }
}

impl De200Controller {
    /// Execute PLAY while preserving Sony-style resume-after-stop semantics.
    ///
    /// With no resume memory, the port receives only `play()`. With resume
    /// memory, `seek(saved_position)` must succeed before `play()` is attempted.
    /// Appliance transport commits to Playing only after all required backend
    /// effects succeed, so a failed seek or play leaves the controller Stopped.
    pub fn request_play_with<P>(&mut self, port: &mut P) -> Result<(), ResumePlayError<P::Error>>
    where
        P: ResumePlaybackPort,
    {
        self.validate_play_request()?;

        if let Some(position) = self.resume_position() {
            port.seek(position).map_err(ResumePlayError::Backend)?;
        }

        port.play().map_err(ResumePlayError::Backend)?;
        self.commit_playing();
        Ok(())
    }
}
