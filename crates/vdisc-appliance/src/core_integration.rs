use std::{error::Error, fmt, path::Path};

use vdisc_core::{
    Player, PlayerError, PlayerState,
    format::{FormatError, FormatErrorKind},
};

#[cfg(target_os = "linux")]
use vdisc_core::PlaybackError;

use crate::{
    De200Controller, DiscFailureClass, DiscState, DiscTransitionError, LcdBackendFacts, LidState,
    NavigationAction, NavigationError, NavigationPlaybackPort, NavigationTransitionError,
    PlayModeTransitionError, PlaybackFailureClass, PlaybackPosition, ResumePlayError,
    ResumePlaybackPort, TrackCompletionIntent, TransportState, TransportTransitionError,
};

/// Concrete Phase-2 bridge between the D-E200 appliance controller and the
/// completed Campaign-01 state backend.
///
/// The bridge owns `vdisc_core::Player` so backend track/position/disc truth is
/// not duplicated in the appliance controller. Real audible Linux output is a
/// Level-3 concern in the Phase-2 workflow; this bridge is the required Level-2
/// controller <-> core <-> real `.vdisc` integration boundary.
#[derive(Debug, Default)]
pub struct CorePlayerBridge {
    player: Player,
}

/// Integration failures preserve the original typed controller/backend error
/// for diagnostics while the physical LCD receives only `ApplianceErrorState`.
#[derive(Debug)]
pub enum CoreIntegrationError {
    DiscTransition(DiscTransitionError),
    TransportTransition(TransportTransitionError),
    PlayModeTransition(PlayModeTransitionError),
    Resume(ResumePlayError<PlayerError>),
    Navigation(NavigationError<PlayerError>),
    Backend(PlayerError),
    TransportDesynchronized {
        appliance: TransportState,
        backend: PlayerState,
    },
    MechanicalDesynchronized {
        lid: LidState,
        disc: DiscState,
        backend: PlayerState,
    },
    MissingShuffleTarget,
    InvalidShuffleTarget {
        target: usize,
        track_count: usize,
        current: usize,
    },
}

impl fmt::Display for CoreIntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiscTransition(error) => write!(f, "disc transition rejected: {error}"),
            Self::TransportTransition(error) => write!(f, "transport transition rejected: {error}"),
            Self::PlayModeTransition(error) => write!(f, "play-mode transition rejected: {error}"),
            Self::Resume(error) => write!(f, "resume integration failed: {error}"),
            Self::Navigation(error) => write!(f, "navigation integration failed: {error}"),
            Self::Backend(error) => write!(f, "vdisc-core operation failed: {error}"),
            Self::TransportDesynchronized { appliance, backend } => write!(
                f,
                "appliance/backend transport desynchronized: appliance={appliance:?}, backend={backend:?}"
            ),
            Self::MechanicalDesynchronized { lid, disc, backend } => write!(
                f,
                "appliance/backend disc state desynchronized: lid={lid:?}, disc={disc:?}, backend={backend:?}"
            ),
            Self::MissingShuffleTarget => {
                write!(
                    f,
                    "RepeatShuffle requires an externally selected track index"
                )
            }
            Self::InvalidShuffleTarget {
                target,
                track_count,
                current,
            } => write!(
                f,
                "shuffle target {target} is invalid for {track_count} tracks while current track is {current}"
            ),
        }
    }
}

impl Error for CoreIntegrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DiscTransition(error) => Some(error),
            Self::TransportTransition(error) => Some(error),
            Self::PlayModeTransition(error) => Some(error),
            Self::Resume(error) => Some(error),
            Self::Navigation(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::TransportDesynchronized { .. }
            | Self::MechanicalDesynchronized { .. }
            | Self::MissingShuffleTarget
            | Self::InvalidShuffleTarget { .. } => None,
        }
    }
}

impl From<DiscTransitionError> for CoreIntegrationError {
    fn from(value: DiscTransitionError) -> Self {
        Self::DiscTransition(value)
    }
}

impl From<TransportTransitionError> for CoreIntegrationError {
    fn from(value: TransportTransitionError) -> Self {
        Self::TransportTransition(value)
    }
}

impl From<NavigationTransitionError> for CoreIntegrationError {
    fn from(value: NavigationTransitionError) -> Self {
        Self::Navigation(NavigationError::Transition(value))
    }
}

impl From<PlayModeTransitionError> for CoreIntegrationError {
    fn from(value: PlayModeTransitionError) -> Self {
        Self::PlayModeTransition(value)
    }
}

/// Classify the Campaign-01 format error without exposing its raw text to the
/// appliance surface.
///
/// `ResourceLimit` predates the appliance taxonomy and contains two meanings.
/// Fixed VDISC/profile limits are invalid content; runtime inability to obtain
/// workers/memory is an unreadable/resource failure. This explicit adapter is
/// the Objective-13 resolution of that mismatch.
pub fn classify_format_error(error: &FormatError) -> DiscFailureClass {
    match error.kind {
        FormatErrorKind::Io => DiscFailureClass::ReadFailure,
        FormatErrorKind::ResourceLimit if is_runtime_resource_failure(&error.message) => {
            DiscFailureClass::ResourceFailure
        }
        FormatErrorKind::ResourceLimit
        | FormatErrorKind::UnsupportedVersion
        | FormatErrorKind::InvalidJson
        | FormatErrorKind::InvalidSchema
        | FormatErrorKind::UnsafePath
        | FormatErrorKind::DuplicateEntry
        | FormatErrorKind::MissingEntry
        | FormatErrorKind::UnexpectedEntry
        | FormatErrorKind::UnsupportedZipFeature
        | FormatErrorKind::MalformedArchive
        | FormatErrorKind::IntegrityMismatch
        | FormatErrorKind::InvalidArtwork
        | FormatErrorKind::UnsupportedMedia => DiscFailureClass::InvalidContent,
    }
}

fn is_runtime_resource_failure(message: &str) -> bool {
    [
        "worker unavailable",
        "bounded media worker requires Linux",
        "cannot locate image worker",
        "allocation failed",
        "entry cannot fit in memory",
        "resource budget",
        "allocation budget",
    ]
    .iter()
    .any(|marker| message.contains(marker))
}

pub fn classify_player_error(error: &PlayerError) -> PlaybackFailureClass {
    match error {
        PlayerError::Disc(error) => PlaybackFailureClass::Disc(classify_format_error(error)),
        PlayerError::InvalidTransition { .. }
        | PlayerError::Boundary { .. }
        | PlayerError::SeekOutOfRange { .. } => PlaybackFailureClass::TransportFailure,
    }
}

#[cfg(target_os = "linux")]
pub fn classify_playback_error(error: &PlaybackError) -> PlaybackFailureClass {
    match error {
        PlaybackError::Player(error) => classify_player_error(error),
        PlaybackError::Disc(error) => PlaybackFailureClass::Disc(classify_format_error(error)),
        PlaybackError::NoOutputDevice => PlaybackFailureClass::NoOutputDevice,
        PlaybackError::UnsupportedOutput { .. } => PlaybackFailureClass::UnsupportedOutput,
        PlaybackError::Device(_) => PlaybackFailureClass::DeviceFailure,
        PlaybackError::Decode(_) => PlaybackFailureClass::DecodeFailure,
        PlaybackError::BackendInvariant(_) => PlaybackFailureClass::BackendInvariant,
    }
}

impl CorePlayerBridge {
    pub fn new() -> Self {
        Self::default()
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

    pub fn position(&self) -> PlaybackPosition {
        PlaybackPosition::from_millis(self.player.position_ms())
    }

    /// Current application gain selected by the appliance policy.
    ///
    /// The Campaign-01 state player intentionally owns no audio gain. Phase-3's
    /// Linux audio bridge consumes this value when connecting the real output
    /// session; keeping it here documents the exact layer boundary without
    /// inventing system-wide mixer behavior.
    pub fn application_gain(&self, controller: &De200Controller) -> f32 {
        controller.volume().normalized()
    }

    /// Validate the physical disc currently in `Inserting` through the real
    /// Campaign-01 `.vdisc` reader/player.
    pub fn validate_inserting_disc(
        &mut self,
        controller: &mut De200Controller,
        path: impl AsRef<Path>,
    ) -> Result<(), CoreIntegrationError> {
        if controller.lid_state() != LidState::Open
            || controller.disc_state() != DiscState::Inserting
            || self.player.state() != PlayerState::Empty
        {
            return Err(CoreIntegrationError::MechanicalDesynchronized {
                lid: controller.lid_state(),
                disc: controller.disc_state(),
                backend: self.player.state(),
            });
        }

        match self.player.insert(path) {
            Ok(()) => {
                controller.notify_disc_validation_accepted()?;
                controller.clear_error_state();
                Ok(())
            }
            Err(error) => {
                if let PlayerError::Disc(format_error) = &error {
                    controller
                        .notify_disc_validation_failed(classify_format_error(format_error))?;
                } else {
                    controller.report_playback_failure(classify_player_error(&error));
                }
                Err(CoreIntegrationError::Backend(error))
            }
        }
    }

    /// Eject the core disc only when the physical removal handshake is waiting
    /// for its completion event, then commit `DiscState::Absent`.
    pub fn complete_disc_removal(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), CoreIntegrationError> {
        if controller.lid_state() != LidState::Open
            || controller.disc_state() != DiscState::Removing
            || controller.transport_state() != TransportState::Stopped
            || self.player.state() != PlayerState::Stopped
        {
            return Err(CoreIntegrationError::MechanicalDesynchronized {
                lid: controller.lid_state(),
                disc: controller.disc_state(),
                backend: self.player.state(),
            });
        }

        self.player.eject().map_err(|error| {
            controller.report_playback_failure(classify_player_error(&error));
            CoreIntegrationError::Backend(error)
        })?;
        controller.notify_disc_removed()?;
        controller.clear_error_state();
        Ok(())
    }

    pub fn play(&mut self, controller: &mut De200Controller) -> Result<(), CoreIntegrationError> {
        controller.validate_play_request()?;
        self.ensure_transport_sync(controller)?;

        let result = {
            let mut port = CorePlayerPort {
                player: &mut self.player,
            };
            controller.request_play_with(&mut port)
        };

        match result {
            Ok(()) => {
                controller.clear_error_state();
                Ok(())
            }
            Err(error) => {
                if let ResumePlayError::Backend(backend) = &error {
                    controller.report_playback_failure(classify_player_error(backend));
                }
                Err(CoreIntegrationError::Resume(error))
            }
        }
    }

    pub fn pause(&mut self, controller: &mut De200Controller) -> Result<(), CoreIntegrationError> {
        controller.validate_pause_request()?;
        self.ensure_transport_sync(controller)?;

        let backend_result = match controller.transport_state() {
            TransportState::Playing => self.player.pause(),
            TransportState::Paused => self.player.play(),
            _ => unreachable!("pause validation accepted an invalid state"),
        };

        if let Err(error) = backend_result {
            controller.report_playback_failure(classify_player_error(&error));
            return Err(CoreIntegrationError::Backend(error));
        }

        controller.commit_pause_toggle();
        controller.clear_error_state();
        Ok(())
    }

    pub fn stop(&mut self, controller: &mut De200Controller) -> Result<(), CoreIntegrationError> {
        controller.validate_stop_request()?;
        self.ensure_transport_sync(controller)?;
        let position = self.position();

        if let Err(error) = self.player.stop() {
            controller.report_playback_failure(classify_player_error(&error));
            return Err(CoreIntegrationError::Backend(error));
        }

        controller.commit_stopped_with_resume(position);
        controller.clear_error_state();
        Ok(())
    }

    pub fn next(&mut self, controller: &mut De200Controller) -> Result<(), CoreIntegrationError> {
        controller.validate_ams_request(NavigationAction::Next)?;
        self.ensure_transport_sync(controller)?;
        let result = {
            let mut port = CorePlayerPort {
                player: &mut self.player,
            };
            controller.request_next_with(&mut port)
        };
        self.finish_navigation(controller, result)
    }

    pub fn previous(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), CoreIntegrationError> {
        controller.validate_ams_request(NavigationAction::Previous)?;
        self.ensure_transport_sync(controller)?;
        let current = if controller.transport_state() == TransportState::Stopped {
            controller
                .resume_position()
                .unwrap_or_else(|| self.position())
        } else {
            self.position()
        };
        let result = {
            let mut port = CorePlayerPort {
                player: &mut self.player,
            };
            controller.request_previous_with(&mut port, current)
        };
        self.finish_navigation(controller, result)
    }

    pub fn scan_seek(
        &mut self,
        controller: &mut De200Controller,
        target: PlaybackPosition,
    ) -> Result<(), CoreIntegrationError> {
        self.ensure_transport_sync(controller)?;
        let current = self.position();
        let result = {
            let mut port = CorePlayerPort {
                player: &mut self.player,
            };
            controller.request_scan_seek_with(&mut port, current, target)
        };
        self.finish_navigation(controller, result)
    }

    /// Execute the already-locked D-E200 end-of-track policy against the real
    /// core state player. RepeatShuffle receives its zero-based target from the
    /// runtime; no RNG is embedded here.
    pub fn track_finished(
        &mut self,
        controller: &mut De200Controller,
        shuffle_target: Option<usize>,
    ) -> Result<TrackCompletionIntent, CoreIntegrationError> {
        self.ensure_transport_sync(controller)?;

        let current = self.player.current_track_index().ok_or(
            CoreIntegrationError::TransportDesynchronized {
                appliance: controller.transport_state(),
                backend: self.player.state(),
            },
        )?;
        let track_count = self.player.track_count();
        let is_last = current + 1 == track_count;
        let intent = controller.track_completion_intent(is_last)?;

        let result = match intent {
            TrackCompletionIntent::Advance => self.player.track_finished(),
            TrackCompletionIntent::StopAtDiscEnd => self.player.track_finished(),
            TrackCompletionIntent::StopAfterCurrent => self.player.stop(),
            TrackCompletionIntent::RestartDisc => self
                .player
                .track_finished()
                .and_then(|()| self.player.play()),
            TrackCompletionIntent::ReplayCurrent => self.player.seek(0),
            TrackCompletionIntent::ChooseShuffleTrack => {
                let target = shuffle_target.ok_or(CoreIntegrationError::MissingShuffleTarget)?;
                if target >= track_count || (track_count > 1 && target == current) {
                    return Err(CoreIntegrationError::InvalidShuffleTarget {
                        target,
                        track_count,
                        current,
                    });
                }
                self.select_track(target)
            }
        };

        if let Err(error) = result {
            self.synchronize_after_backend_failure(controller);
            controller.report_playback_failure(classify_player_error(&error));
            return Err(CoreIntegrationError::Backend(error));
        }

        if matches!(
            intent,
            TrackCompletionIntent::StopAtDiscEnd | TrackCompletionIntent::StopAfterCurrent
        ) {
            controller.commit_backend_stopped_without_resume();
        }

        controller.clear_error_state();
        Ok(intent)
    }

    pub fn lcd_facts(&self) -> LcdBackendFacts {
        if self.player.state() == PlayerState::Empty {
            return LcdBackendFacts::default();
        }

        let current_track_number = self
            .player
            .current_track_index()
            .and_then(|index| index.checked_add(1))
            .and_then(|number| u8::try_from(number).ok());
        let total_tracks = u8::try_from(self.player.track_count()).ok();
        let total_time = self.player.disc().and_then(|disc| {
            disc.tracks()
                .iter()
                .try_fold(0u64, |sum, track| sum.checked_add(track.duration_ms?))
        });

        LcdBackendFacts::new(
            current_track_number,
            Some(self.position()),
            total_tracks,
            total_time.map(PlaybackPosition::from_millis),
        )
    }

    fn finish_navigation(
        &mut self,
        controller: &mut De200Controller,
        result: Result<(), NavigationError<PlayerError>>,
    ) -> Result<(), CoreIntegrationError> {
        match result {
            Ok(()) => {
                controller.clear_error_state();
                Ok(())
            }
            Err(error) => {
                if let NavigationError::Backend(backend) = &error
                    && !matches!(
                        backend,
                        PlayerError::Boundary { .. } | PlayerError::SeekOutOfRange { .. }
                    )
                {
                    controller.report_playback_failure(classify_player_error(backend));
                }
                Err(CoreIntegrationError::Navigation(error))
            }
        }
    }

    fn select_track(&mut self, target: usize) -> Result<(), PlayerError> {
        let current =
            self.player
                .current_track_index()
                .ok_or_else(|| PlayerError::InvalidTransition {
                    action: vdisc_core::PlayerAction::Next,
                    state: self.player.state(),
                })?;

        if target > current {
            for _ in current..target {
                self.player.next_track()?;
            }
        } else if target < current {
            for _ in target..current {
                self.player.previous()?;
            }
        } else {
            self.player.seek(0)?;
        }
        Ok(())
    }

    fn ensure_transport_sync(
        &self,
        controller: &De200Controller,
    ) -> Result<(), CoreIntegrationError> {
        let matches = match controller.transport_state() {
            TransportState::Stopped => self.player.state() == PlayerState::Stopped,
            TransportState::Playing
            | TransportState::SeekingForward
            | TransportState::SeekingBackward => self.player.state() == PlayerState::Playing,
            TransportState::Paused => self.player.state() == PlayerState::Paused,
        };

        if matches {
            Ok(())
        } else {
            Err(CoreIntegrationError::TransportDesynchronized {
                appliance: controller.transport_state(),
                backend: self.player.state(),
            })
        }
    }

    fn synchronize_after_backend_failure(&self, controller: &mut De200Controller) {
        if self.player.state() == PlayerState::Stopped
            && controller.transport_state() != TransportState::Stopped
        {
            controller.commit_backend_stopped_without_resume();
        }
    }
}

struct CorePlayerPort<'a> {
    player: &'a mut Player,
}

impl ResumePlaybackPort for CorePlayerPort<'_> {
    type Error = PlayerError;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        if self.player.state() == PlayerState::Stopped {
            self.player.restore_stopped_position(position.as_millis())
        } else {
            self.player.seek(position.as_millis())
        }
    }

    fn play(&mut self) -> Result<(), Self::Error> {
        self.player.play()
    }
}

impl NavigationPlaybackPort for CorePlayerPort<'_> {
    type Error = PlayerError;

    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        if self.player.state() == PlayerState::Stopped {
            self.player.restore_stopped_position(position.as_millis())
        } else {
            self.player.seek(position.as_millis())
        }
    }

    fn next_track(&mut self) -> Result<(), Self::Error> {
        self.player.next_track()
    }

    fn previous_track(&mut self) -> Result<(), Self::Error> {
        self.player.previous()
    }
}
