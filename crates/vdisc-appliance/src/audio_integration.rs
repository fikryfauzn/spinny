use crate::{
    CoreIntegrationError, De200Controller, DiscState, LcdBackendFacts, LidState, NavigationError,
    PlaybackPosition, ResumePlayError, TransportState, classify_format_error,
    classify_playback_error, classify_player_error,
};
use std::{error::Error, fmt, path::Path};
use vdisc_core::{
    BurnedDisc, PlaybackBackend, PlaybackError, PlaybackSession, Player, PlayerError, PlayerState,
};

mod events;
mod transport;

/// Appliance policy and a single validated disc composed with real output.
pub struct AudioPlayerBridge<B: PlaybackBackend> {
    player: Player,
    backend: B,
    session: Option<B::Session>,
    pending_eof: bool,
    verification_failure_reported: bool,
    rng: fastrand::Rng,
}

#[derive(Debug)]
pub enum AudioIntegrationError {
    Policy(CoreIntegrationError),
    Resume(ResumePlayError<PlaybackError>),
    Navigation(NavigationError<PlaybackError>),
    Playback(PlaybackError),
    VerificationPending,
}
impl fmt::Display for AudioIntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Policy(e) => e.fmt(f),
            Self::Resume(e) => e.fmt(f),
            Self::Navigation(e) => e.fmt(f),
            Self::Playback(e) => e.fmt(f),
            Self::VerificationPending => write!(f, "requested track is still verifying"),
        }
    }
}
impl Error for AudioIntegrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Policy(e) => Some(e as &(dyn Error + 'static)),
            Self::Resume(e) => Some(e as &(dyn Error + 'static)),
            Self::Navigation(e) => Some(e as &(dyn Error + 'static)),
            Self::Playback(e) => Some(e as &(dyn Error + 'static)),
            Self::VerificationPending => None,
        }
    }
}
impl From<CoreIntegrationError> for AudioIntegrationError {
    fn from(error: CoreIntegrationError) -> Self {
        Self::Policy(error)
    }
}

impl<B: PlaybackBackend> AudioPlayerBridge<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            player: Player::new(),
            backend,
            session: None,
            pending_eof: false,
            verification_failure_reported: false,
            rng: fastrand::Rng::new(),
        }
    }
    pub fn with_backend_and_seed(backend: B, seed: u64) -> Self {
        Self {
            rng: fastrand::Rng::with_seed(seed),
            ..Self::with_backend(backend)
        }
    }
    pub fn backend_state(&self) -> PlayerState {
        self.player.state()
    }
    pub fn current_track_index(&self) -> Option<usize> {
        self.player.current_track_index()
    }
    pub fn track_count(&self) -> usize {
        self.player.track_count()
    }
    pub fn current_track_duration_ms(&self) -> Option<u64> {
        self.player
            .current_track()
            .and_then(|track| track.duration_ms)
    }
    pub fn position(&self) -> PlaybackPosition {
        PlaybackPosition::from_millis(
            self.session
                .as_ref()
                .map_or_else(|| self.player.position_ms(), PlaybackSession::position_ms),
        )
    }
    pub fn application_gain(&self, controller: &De200Controller) -> f32 {
        controller.volume().normalized()
    }
    pub fn lcd_facts(&self) -> LcdBackendFacts {
        if self.player.state() == PlayerState::Empty {
            return LcdBackendFacts::default();
        }
        let track = self
            .current_track_index()
            .and_then(|i| u8::try_from(i + 1).ok());
        let count = u8::try_from(self.track_count()).ok();
        let total = self.player.disc().and_then(|disc| {
            disc.tracks()
                .iter()
                .try_fold(0u64, |sum, track| sum.checked_add(track.duration_ms?))
        });
        LcdBackendFacts::new(
            track,
            Some(self.position()),
            count,
            total.map(PlaybackPosition::from_millis),
        )
    }
    pub fn validate_inserting_disc(
        &mut self,
        controller: &mut De200Controller,
        path: impl AsRef<Path>,
    ) -> Result<(), AudioIntegrationError> {
        if controller.lid_state() != LidState::Open
            || controller.disc_state() != DiscState::Inserting
            || self.player.state() != PlayerState::Empty
        {
            return Err(self.mechanical_error(controller));
        }
        match self.player.insert(path) {
            Ok(()) => {
                self.verification_failure_reported = false;
                controller
                    .notify_disc_validation_accepted()
                    .map_err(CoreIntegrationError::from)?;
                controller.clear_error_state();
                Ok(())
            }
            Err(error) => {
                if let PlayerError::Disc(format) = &error {
                    controller
                        .notify_disc_validation_failed(classify_format_error(format))
                        .map_err(CoreIntegrationError::from)?;
                } else {
                    controller.report_playback_failure(classify_player_error(&error));
                }
                Err(AudioIntegrationError::Playback(error.into()))
            }
        }
    }
    /// Commit a disc verified on a worker thread. The GUI thread owns the
    /// player/controller mutation, and this must not revalidate the archive.
    pub fn accept_validated_disc(
        &mut self,
        controller: &mut De200Controller,
        disc: BurnedDisc,
    ) -> Result<(), AudioIntegrationError> {
        if controller.lid_state() != LidState::Open
            || controller.disc_state() != DiscState::Inserting
            || self.player.state() != PlayerState::Empty
        {
            return Err(self.mechanical_error(controller));
        }
        self.player.insert_validated(disc).map_err(|error| {
            controller.report_playback_failure(classify_player_error(&error));
            AudioIntegrationError::Playback(error.into())
        })?;
        self.verification_failure_reported = false;
        controller
            .notify_disc_validation_accepted()
            .map_err(CoreIntegrationError::from)?;
        controller.clear_error_state();
        Ok(())
    }

    pub fn complete_disc_removal(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), AudioIntegrationError> {
        if controller.lid_state() != LidState::Open
            || controller.disc_state() != DiscState::Removing
            || controller.transport_state() != TransportState::Stopped
            || self.player.state() != PlayerState::Stopped
        {
            return Err(self.mechanical_error(controller));
        }
        self.retire_session();
        // The retired verifier owns only a pinned file descriptor and a
        // cancellation flag. It cannot report into the next disc session.
        if let Some(disc) = self.player.disc() {
            disc.cancel_background_verification();
        }
        if let Err(error) = self.player.eject() {
            return Err(self.fail_backend(controller, error.into()));
        }
        self.verification_failure_reported = false;
        controller
            .notify_disc_removed()
            .map_err(CoreIntegrationError::from)?;
        controller.clear_error_state();
        Ok(())
    }
    /// Prevent Qt navigation from waiting on a background SHA-256 pass.
    /// A pending track is a controlled user rejection, not a machine failure.
    /// A completed, failed validation is a real disc failure.
    fn playback_track_ready(
        &mut self,
        controller: &mut De200Controller,
        index: usize,
    ) -> Result<bool, AudioIntegrationError> {
        let disc = self.player.disc().ok_or(AudioIntegrationError::Playback(
            PlaybackError::BackendInvariant("track readiness requested without an inserted disc"),
        ))?;
        match disc.playback_track_ready(index) {
            Ok(ready) => Ok(ready),
            Err(error) => Err(self.fail_backend(controller, PlaybackError::Disc(error))),
        }
    }

    fn mechanical_error(&self, controller: &De200Controller) -> AudioIntegrationError {
        CoreIntegrationError::MechanicalDesynchronized {
            lid: controller.lid_state(),
            disc: controller.disc_state(),
            backend: self.player.state(),
        }
        .into()
    }
    fn ensure_transport_sync(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), AudioIntegrationError> {
        let expected = match controller.transport_state() {
            TransportState::Stopped => PlayerState::Stopped,
            TransportState::Paused => PlayerState::Paused,
            _ => PlayerState::Playing,
        };
        if self.player.state() == expected
            && (expected == PlayerState::Stopped || self.session.is_some())
        {
            Ok(())
        } else {
            Err(self.fail_backend(
                controller,
                PlaybackError::BackendInvariant(
                    "appliance transport and audio session are desynchronized",
                ),
            ))
        }
    }
    fn retire_session(&mut self) {
        self.session = None;
        self.pending_eof = false;
    }
    fn fail_backend(
        &mut self,
        controller: &mut De200Controller,
        error: PlaybackError,
    ) -> AudioIntegrationError {
        self.retire_session();
        if self.player.state() != PlayerState::Empty {
            let _ = self.player.stop();
        }
        controller.commit_backend_stopped_without_resume();
        controller.report_playback_failure(classify_playback_error(&error));
        AudioIntegrationError::Playback(error)
    }
    fn handle_operation(
        &mut self,
        controller: &mut De200Controller,
        result: Result<(), PlaybackError>,
    ) -> Result<(), AudioIntegrationError> {
        result.map_err(|error| self.fail_backend(controller, error))
    }
    /// Retiring before opening bounds both audible overlap and decoder resources.
    fn replace_session(&mut self, gain: f32) -> Result<(), PlaybackError> {
        self.retire_session();
        let track = self
            .current_track_index()
            .ok_or(PlaybackError::BackendInvariant("missing selected track"))?;
        let disc = self
            .player
            .disc()
            .ok_or(PlaybackError::BackendInvariant("missing validated disc"))?;
        let session = self
            .backend
            .open_session(disc, track, self.player.position_ms())?;
        session.set_gain(gain)?;
        if self.player.state() == PlayerState::Playing {
            session.play()?;
        }
        self.session = Some(session);
        Ok(())
    }
}
