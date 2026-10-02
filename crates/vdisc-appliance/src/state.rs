use std::{error::Error, fmt};

/// Mechanical lid state of the D-E200 appliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidState {
    Closed,
    Opening,
    Open,
    Closing,
}

/// Physical disc-presence state inside the appliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscState {
    Absent,
    Inserting,
    Seated,
    Removing,
}

/// Appliance-level interpretation of transport activity.
///
/// This is intentionally not the same type as `vdisc_core::PlayerState`.
/// Seeking states describe physical-control/choreography behavior that the
/// backend player does not need to own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
    SeekingForward,
    SeekingBackward,
}

/// Sony-style playback policy selected by the appliance MENU control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Normal,
    RepeatAll,
    Single,
    RepeatSingle,
    RepeatShuffle,
}

/// Normalized application playback gain.
///
/// Objective 1 establishes only the valid range. It deliberately does not
/// define button step count, gain curve, persistence, or AVLS clamping policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Volume(f32);

impl Volume {
    pub const MIN: f32 = 0.0;
    pub const MAX: f32 = 1.0;

    pub fn new(value: f32) -> Result<Self, VolumeError> {
        if !value.is_finite() {
            return Err(VolumeError::NotFinite);
        }

        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(VolumeError::OutOfRange { value });
        }

        Ok(Self(value))
    }

    pub fn normalized(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolumeError {
    NotFinite,
    OutOfRange { value: f32 },
}

impl fmt::Display for VolumeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite => write!(f, "volume must be finite"),
            Self::OutOfRange { value } => write!(
                f,
                "volume {value} is outside the normalized range {}..={}",
                Volume::MIN,
                Volume::MAX
            ),
        }
    }
}

impl Error for VolumeError {}

/// Appliance resume memory expressed in the same millisecond unit exposed by
/// the current backend player API.
///
/// The wrapper keeps raw transport units from leaking through future appliance
/// code. Backend conversion should remain at the adapter boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaybackPosition(u64);

impl PlaybackPosition {
    pub const fn from_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }
}

/// Machine-facing failure categories.
///
/// Detailed backend diagnostics remain outside the physical appliance surface.
/// Objective 12 will define the translation from concrete backend failures.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplianceErrorState {
    InvalidDisc,
    UnreadableDisc,
    PlaybackFailure,
    AudioOutputFailure,
}

/// Explicit inputs to the mechanical lid handshake.
///
/// Request actions begin visual motion. Completion actions are acknowledgements
/// from the future visual layer; the controller never infers completion from
/// elapsed wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidAction {
    RequestOpen,
    Opened,
    RequestClose,
    Closed,
}

/// A lid action was not legal for the current mechanical lid state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LidTransitionError {
    action: LidAction,
    state: LidState,
}

impl LidTransitionError {
    pub const fn action(self) -> LidAction {
        self.action
    }

    pub const fn state(self) -> LidState {
        self.state
    }
}

impl fmt::Display for LidTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lid action {:?} is invalid while lid is {:?}",
            self.action, self.state
        )
    }
}

impl Error for LidTransitionError {}

/// Authoritative state owned by the D-E200 appliance layer.
///
/// Backend-owned facts such as loaded-disc metadata, current track selection,
/// and live playback position are intentionally not duplicated here. Later
/// objectives may project those values into read-only appliance snapshots, but
/// `vdisc-core` remains their source of truth.
#[derive(Debug)]
pub struct De200Controller {
    lid: LidState,
    disc: DiscState,
    transport: TransportState,
    play_mode: PlayMode,
    hold_enabled: bool,
    avls_enabled: bool,
    volume: Volume,
    resume_position: Option<PlaybackPosition>,
    error: Option<ApplianceErrorState>,
}

impl De200Controller {
    /// Construct the initial appliance state.
    ///
    /// Initial volume is supplied by the application because the Phase 2
    /// contract intentionally does not define a default volume policy.
    pub const fn new(initial_volume: Volume) -> Self {
        Self {
            lid: LidState::Closed,
            disc: DiscState::Absent,
            transport: TransportState::Stopped,
            play_mode: PlayMode::Normal,
            hold_enabled: false,
            avls_enabled: false,
            volume: initial_volume,
            resume_position: None,
            error: None,
        }
    }

    pub const fn lid_state(&self) -> LidState {
        self.lid
    }

    pub const fn disc_state(&self) -> DiscState {
        self.disc
    }

    pub const fn transport_state(&self) -> TransportState {
        self.transport
    }

    pub const fn play_mode(&self) -> PlayMode {
        self.play_mode
    }

    pub const fn hold_enabled(&self) -> bool {
        self.hold_enabled
    }

    pub const fn avls_enabled(&self) -> bool {
        self.avls_enabled
    }

    pub const fn volume(&self) -> Volume {
        self.volume
    }

    pub const fn resume_position(&self) -> Option<PlaybackPosition> {
        self.resume_position
    }

    pub const fn error_state(&self) -> Option<ApplianceErrorState> {
        self.error
    }

    /// Begin opening the lid.
    ///
    /// Objective 2 deliberately checks only mechanical lid sequencing. The
    /// transport OPEN interlock is Objective 4 and is not smuggled in here.
    pub fn request_lid_open(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::RequestOpen, LidState::Closed, LidState::Opening)
    }

    /// Acknowledge that the visual/mechanical opening motion completed.
    pub fn notify_lid_opened(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::Opened, LidState::Opening, LidState::Open)
    }

    /// Begin closing the lid.
    pub fn request_lid_close(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::RequestClose, LidState::Open, LidState::Closing)
    }

    /// Acknowledge that the visual/mechanical closing motion completed.
    pub fn notify_lid_closed(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::Closed, LidState::Closing, LidState::Closed)
    }

    fn transition_lid(
        &mut self,
        action: LidAction,
        expected: LidState,
        next: LidState,
    ) -> Result<(), LidTransitionError> {
        if self.lid != expected {
            return Err(LidTransitionError {
                action,
                state: self.lid,
            });
        }

        self.lid = next;
        Ok(())
    }
}
