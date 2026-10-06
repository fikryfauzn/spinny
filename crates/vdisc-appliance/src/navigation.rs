use std::{error::Error, fmt};

use crate::state::{
    De200Controller, NavigationAction, NavigationTransitionError, PlaybackPosition,
};

/// Backend-neutral capabilities required by the D-E200 AMS controls.
///
/// Objective 7 deliberately keeps real `vdisc-core` integration deferred. The
/// appliance controller chooses the physical meaning of the button press while
/// the port performs the corresponding backend navigation primitive.
pub trait NavigationPlaybackPort {
    type Error;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error>;
    fn next_track(&mut self) -> Result<(), Self::Error>;
    fn previous_track(&mut self) -> Result<(), Self::Error>;
}

/// Failure while executing an AMS or held-scan request.
#[derive(Debug, PartialEq, Eq)]
pub enum NavigationError<E> {
    Transition(NavigationTransitionError),
    Backend(E),
}

impl<E> From<NavigationTransitionError> for NavigationError<E> {
    fn from(error: NavigationTransitionError) -> Self {
        Self::Transition(error)
    }
}

impl<E: fmt::Display> fmt::Display for NavigationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transition(error) => write!(f, "navigation request rejected: {error}"),
            Self::Backend(error) => write!(f, "navigation backend failed: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for NavigationError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transition(error) => Some(error),
            Self::Backend(error) => Some(error),
        }
    }
}

impl De200Controller {
    /// Execute short NEXT AMS navigation.
    ///
    /// Transport state is preserved. Resume memory is cleared only after the
    /// backend confirms the track change, preventing a stale stop-resume point
    /// from belonging to a different selected track.
    pub fn request_next_with<P>(&mut self, port: &mut P) -> Result<(), NavigationError<P::Error>>
    where
        P: NavigationPlaybackPort,
    {
        self.validate_ams_request(NavigationAction::Next)?;
        port.next_track().map_err(NavigationError::Backend)?;
        self.clear_resume_position();
        Ok(())
    }

    /// Execute short PREVIOUS using the explicit VDISC AMS rule.
    ///
    /// VDISC usability policy (not a claimed Sony hardware threshold):
    /// - position < 3000 ms: move to the previous track;
    /// - position >= 3000 ms: restart the current track at 0.
    ///
    /// Live position remains backend-owned and is supplied at the adapter
    /// boundary. Successful navigation clears old stop-resume memory.
    pub fn request_previous_with<P>(
        &mut self,
        port: &mut P,
        current_position: PlaybackPosition,
    ) -> Result<(), NavigationError<P::Error>>
    where
        P: NavigationPlaybackPort,
    {
        self.validate_ams_request(NavigationAction::Previous)?;

        if current_position.as_millis() < 3_000 {
            port.previous_track().map_err(NavigationError::Backend)?;
        } else {
            port.seek(PlaybackPosition::from_millis(0))
                .map_err(NavigationError::Backend)?;
        }

        self.clear_resume_position();
        Ok(())
    }

    /// Execute one seek target while a physical AMS control remains held.
    ///
    /// Objective 7 intentionally does not choose scan speed or cadence. The
    /// runtime may generate repeated targets at whatever cadence is later
    /// validated, while the controller enforces that targets move in the held
    /// direction and that scan is active.
    pub fn request_scan_seek_with<P>(
        &mut self,
        port: &mut P,
        current_position: PlaybackPosition,
        target_position: PlaybackPosition,
    ) -> Result<(), NavigationError<P::Error>>
    where
        P: NavigationPlaybackPort,
    {
        self.validate_scan_seek_request(current_position, target_position)?;
        port.seek(target_position)
            .map_err(NavigationError::Backend)?;
        self.clear_resume_position();
        Ok(())
    }
}
