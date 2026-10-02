use crate::state::{
    ApplianceErrorState, De200Controller, PlayMode, PlaybackPosition, TransportState,
};

/// Backend-owned facts that may be projected onto the physical LCD.
///
/// Track number is one-based. The appliance does not own or persist any of
/// these values; the runtime/backend adapter supplies the current snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LcdBackendFacts {
    current_track_number: Option<u8>,
    elapsed_time: Option<PlaybackPosition>,
    total_tracks: Option<u8>,
    total_time: Option<PlaybackPosition>,
}

impl LcdBackendFacts {
    pub const fn new(
        current_track_number: Option<u8>,
        elapsed_time: Option<PlaybackPosition>,
        total_tracks: Option<u8>,
        total_time: Option<PlaybackPosition>,
    ) -> Self {
        Self {
            current_track_number,
            elapsed_time,
            total_tracks,
            total_time,
        }
    }

    pub const fn current_track_number(self) -> Option<u8> {
        self.current_track_number
    }

    pub const fn elapsed_time(self) -> Option<PlaybackPosition> {
        self.elapsed_time
    }

    pub const fn total_tracks(self) -> Option<u8> {
        self.total_tracks
    }

    pub const fn total_time(self) -> Option<PlaybackPosition> {
        self.total_time
    }
}

/// Transport meaning exposed to an LCD renderer.
///
/// This intentionally avoids coupling the future renderer directly to the
/// controller's internal state layout while preserving the semantic transport
/// states defined by the Phase 2 appliance contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdPlaybackStatus {
    Stopped,
    Playing,
    Paused,
    SeekingForward,
    SeekingBackward,
}

impl From<TransportState> for LcdPlaybackStatus {
    fn from(value: TransportState) -> Self {
        match value {
            TransportState::Stopped => Self::Stopped,
            TransportState::Playing => Self::Playing,
            TransportState::Paused => Self::Paused,
            TransportState::SeekingForward => Self::SeekingForward,
            TransportState::SeekingBackward => Self::SeekingBackward,
        }
    }
}

/// Runtime-owned transient machine feedback.
///
/// Objective 11 defines only semantic priority. Exact display duration is a
/// later UI/runtime concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdTransientMessage {
    Hold,
}

/// Constrained physical-LCD message vocabulary.
///
/// Detailed diagnostics and raw Rust/backend errors are intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdMessage {
    Hold,
    InvalidDisc,
    UnreadableDisc,
    PlaybackFailure,
    AudioOutputFailure,
}

impl From<LcdTransientMessage> for LcdMessage {
    fn from(value: LcdTransientMessage) -> Self {
        match value {
            LcdTransientMessage::Hold => Self::Hold,
        }
    }
}

impl From<ApplianceErrorState> for LcdMessage {
    fn from(value: ApplianceErrorState) -> Self {
        match value {
            ApplianceErrorState::InvalidDisc => Self::InvalidDisc,
            ApplianceErrorState::UnreadableDisc => Self::UnreadableDisc,
            ApplianceErrorState::PlaybackFailure => Self::PlaybackFailure,
            ApplianceErrorState::AudioOutputFailure => Self::AudioOutputFailure,
        }
    }
}

/// Renderer-facing semantic LCD snapshot.
///
/// A snapshot contains only machine/display facts. It has no pixel geometry,
/// font, animation timing, artwork, title, album, artist, or Shelf metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LcdSnapshot {
    track_number: Option<u8>,
    elapsed_time: Option<PlaybackPosition>,
    total_tracks: Option<u8>,
    total_time: Option<PlaybackPosition>,
    play_mode: PlayMode,
    hold_indicator: bool,
    avls_indicator: bool,
    playback_status: LcdPlaybackStatus,
    message: Option<LcdMessage>,
}

impl LcdSnapshot {
    pub const fn track_number(self) -> Option<u8> {
        self.track_number
    }

    pub const fn elapsed_time(self) -> Option<PlaybackPosition> {
        self.elapsed_time
    }

    pub const fn total_tracks(self) -> Option<u8> {
        self.total_tracks
    }

    pub const fn total_time(self) -> Option<PlaybackPosition> {
        self.total_time
    }

    pub const fn play_mode(self) -> PlayMode {
        self.play_mode
    }

    pub const fn hold_indicator(self) -> bool {
        self.hold_indicator
    }

    pub const fn avls_indicator(self) -> bool {
        self.avls_indicator
    }

    pub const fn playback_status(self) -> LcdPlaybackStatus {
        self.playback_status
    }

    pub const fn message(self) -> Option<LcdMessage> {
        self.message
    }
}

impl De200Controller {
    /// Project current appliance state and backend-owned playback facts into a
    /// deterministic renderer-facing LCD snapshot.
    ///
    /// Priority is:
    ///
    /// 1. machine error already translated to `ApplianceErrorState`;
    /// 2. runtime transient feedback such as `Hold`;
    /// 3. normal track/time facts.
    ///
    /// Messages suppress the normal track/time region but do not erase the
    /// persistent play-mode, HOLD, AVLS, or playback-status indicators. Exact
    /// transient duration and physical LCD geometry remain later runtime work.
    pub fn lcd_snapshot(
        &self,
        backend: LcdBackendFacts,
        transient: Option<LcdTransientMessage>,
    ) -> LcdSnapshot {
        project_lcd(
            self.play_mode(),
            self.hold_enabled(),
            self.avls_enabled(),
            self.transport_state(),
            self.error_state(),
            backend,
            transient,
        )
    }
}

fn project_lcd(
    play_mode: PlayMode,
    hold_enabled: bool,
    avls_enabled: bool,
    transport: TransportState,
    error: Option<ApplianceErrorState>,
    backend: LcdBackendFacts,
    transient: Option<LcdTransientMessage>,
) -> LcdSnapshot {
    let message = error
        .map(LcdMessage::from)
        .or_else(|| transient.map(LcdMessage::from));

    let suppress_track_time = message.is_some();

    LcdSnapshot {
        track_number: (!suppress_track_time)
            .then_some(backend.current_track_number())
            .flatten(),
        elapsed_time: (!suppress_track_time)
            .then_some(backend.elapsed_time())
            .flatten(),
        total_tracks: (!suppress_track_time)
            .then_some(backend.total_tracks())
            .flatten(),
        total_time: (!suppress_track_time)
            .then_some(backend.total_time())
            .flatten(),
        play_mode,
        hold_indicator: hold_enabled,
        avls_indicator: avls_enabled,
        playback_status: transport.into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_backend() -> LcdBackendFacts {
        LcdBackendFacts::new(
            Some(2),
            Some(PlaybackPosition::from_millis(12_345)),
            Some(6),
            Some(PlaybackPosition::from_millis(321_000)),
        )
    }

    #[test]
    fn every_machine_error_maps_to_constrained_lcd_message() {
        let cases = [
            (ApplianceErrorState::InvalidDisc, LcdMessage::InvalidDisc),
            (
                ApplianceErrorState::UnreadableDisc,
                LcdMessage::UnreadableDisc,
            ),
            (
                ApplianceErrorState::PlaybackFailure,
                LcdMessage::PlaybackFailure,
            ),
            (
                ApplianceErrorState::AudioOutputFailure,
                LcdMessage::AudioOutputFailure,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(LcdMessage::from(error), expected);
        }
    }

    #[test]
    fn machine_error_has_priority_over_transient_hold_and_track_time() {
        let snapshot = project_lcd(
            PlayMode::RepeatAll,
            true,
            true,
            TransportState::Playing,
            Some(ApplianceErrorState::PlaybackFailure),
            populated_backend(),
            Some(LcdTransientMessage::Hold),
        );

        assert_eq!(snapshot.message(), Some(LcdMessage::PlaybackFailure));
        assert_eq!(snapshot.track_number(), None);
        assert_eq!(snapshot.elapsed_time(), None);
        assert_eq!(snapshot.total_tracks(), None);
        assert_eq!(snapshot.total_time(), None);
        assert_eq!(snapshot.play_mode(), PlayMode::RepeatAll);
        assert!(snapshot.hold_indicator());
        assert!(snapshot.avls_indicator());
        assert_eq!(snapshot.playback_status(), LcdPlaybackStatus::Playing);
    }
}
