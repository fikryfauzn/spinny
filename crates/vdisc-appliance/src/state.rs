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

/// A lid action was not legal for the current appliance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LidTransitionError {
    action: LidAction,
    state: LidState,
    disc_state: Option<DiscState>,
    transport_state: Option<TransportState>,
}

impl LidTransitionError {
    pub const fn action(self) -> LidAction {
        self.action
    }

    pub const fn state(self) -> LidState {
        self.state
    }

    /// Disc state when disc choreography specifically blocked this lid action.
    ///
    /// `None` means the lid state itself made the action invalid.
    pub const fn disc_state(self) -> Option<DiscState> {
        self.disc_state
    }

    /// Transport state when the OPEN interlock specifically blocked opening.
    ///
    /// `None` means transport was not the reason for rejection.
    pub const fn transport_state(self) -> Option<TransportState> {
        self.transport_state
    }
}

impl fmt::Display for LidTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(transport_state) = self.transport_state {
            write!(
                f,
                "lid action {:?} is blocked while transport is {:?}",
                self.action, transport_state
            )
        } else if let Some(disc_state) = self.disc_state {
            write!(
                f,
                "lid action {:?} is invalid while lid is {:?} and disc is {:?}",
                self.action, self.state, disc_state
            )
        } else {
            write!(
                f,
                "lid action {:?} is invalid while lid is {:?}",
                self.action, self.state
            )
        }
    }
}

impl Error for LidTransitionError {}

/// Explicit inputs to the physical disc insertion/removal handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscAction {
    RequestInsert,
    ValidationAccepted,
    ValidationRejected,
    Seated,
    RequestRemove,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscTransitionErrorKind {
    LidNotOpen,
    InvalidDiscState,
    ValidationRequired,
}

/// A disc action was not legal for the current appliance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscTransitionError {
    action: DiscAction,
    lid_state: LidState,
    disc_state: DiscState,
    kind: DiscTransitionErrorKind,
}

impl DiscTransitionError {
    pub const fn action(self) -> DiscAction {
        self.action
    }

    pub const fn lid_state(self) -> LidState {
        self.lid_state
    }

    pub const fn disc_state(self) -> DiscState {
        self.disc_state
    }

    pub const fn kind(self) -> DiscTransitionErrorKind {
        self.kind
    }
}

impl fmt::Display for DiscTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "disc action {:?} is invalid while lid is {:?} and disc is {:?}: {:?}",
            self.action, self.lid_state, self.disc_state, self.kind
        )
    }
}

impl Error for DiscTransitionError {}

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
    insertion_validated: bool,
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
            insertion_validated: false,
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
    /// OPEN is permitted only while transport is fully stopped. The interlock
    /// is enforced before any mechanical motion begins so rejected requests do
    /// not partially mutate appliance state.
    pub fn request_lid_open(&mut self) -> Result<(), LidTransitionError> {
        if self.lid != LidState::Closed {
            return Err(LidTransitionError {
                action: LidAction::RequestOpen,
                state: self.lid,
                disc_state: None,
                transport_state: None,
            });
        }

        if self.transport != TransportState::Stopped {
            return Err(LidTransitionError {
                action: LidAction::RequestOpen,
                state: self.lid,
                disc_state: None,
                transport_state: Some(self.transport),
            });
        }

        self.lid = LidState::Opening;
        Ok(())
    }

    /// Acknowledge that the visual/mechanical opening motion completed.
    pub fn notify_lid_opened(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::Opened, LidState::Opening, LidState::Open)
    }

    /// Begin closing the lid.
    ///
    /// An empty or fully seated disc is valid. Closing while insertion or
    /// removal is still in progress is rejected so visual and logical state
    /// cannot cross each other.
    pub fn request_lid_close(&mut self) -> Result<(), LidTransitionError> {
        if self.lid != LidState::Open {
            return Err(LidTransitionError {
                action: LidAction::RequestClose,
                state: self.lid,
                disc_state: None,
                transport_state: None,
            });
        }

        if !matches!(self.disc, DiscState::Absent | DiscState::Seated) {
            return Err(LidTransitionError {
                action: LidAction::RequestClose,
                state: self.lid,
                disc_state: Some(self.disc),
                transport_state: None,
            });
        }

        self.lid = LidState::Closing;
        Ok(())
    }

    /// Acknowledge that the visual/mechanical closing motion completed.
    pub fn notify_lid_closed(&mut self) -> Result<(), LidTransitionError> {
        self.transition_lid(LidAction::Closed, LidState::Closing, LidState::Closed)
    }

    /// Begin physical insertion of a virtual disc.
    ///
    /// This objective owns only the appliance handshake. Real `.vdisc`
    /// validation remains a backend responsibility and is represented here by
    /// a later explicit validation-success or validation-failure event.
    pub fn request_disc_insert(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::RequestInsert)?;
        self.require_disc_state(DiscAction::RequestInsert, DiscState::Absent)?;

        self.disc = DiscState::Inserting;
        self.insertion_validated = false;
        Ok(())
    }

    /// Record successful validation of the disc currently being inserted.
    ///
    /// The future backend adapter must call this only after `.vdisc`
    /// validation succeeds. Validation alone does not complete physical seating.
    pub fn notify_disc_validation_accepted(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::ValidationAccepted)?;
        self.require_disc_state(DiscAction::ValidationAccepted, DiscState::Inserting)?;

        self.insertion_validated = true;
        Ok(())
    }

    /// Reject the current insertion attempt after validation fails.
    ///
    /// Error translation and LCD messaging remain Objective 12. For now the
    /// controller only guarantees that a rejected disc never becomes seated.
    pub fn notify_disc_validation_rejected(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::ValidationRejected)?;
        self.require_disc_state(DiscAction::ValidationRejected, DiscState::Inserting)?;

        self.disc = DiscState::Absent;
        self.insertion_validated = false;
        Ok(())
    }

    /// Acknowledge that validated insertion has physically completed.
    pub fn notify_disc_seated(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::Seated)?;
        self.require_disc_state(DiscAction::Seated, DiscState::Inserting)?;

        if !self.insertion_validated {
            return Err(self.disc_error(
                DiscAction::Seated,
                DiscTransitionErrorKind::ValidationRequired,
            ));
        }

        self.disc = DiscState::Seated;
        self.insertion_validated = false;
        Ok(())
    }

    /// Begin physical removal of a seated disc.
    pub fn request_disc_remove(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::RequestRemove)?;
        self.require_disc_state(DiscAction::RequestRemove, DiscState::Seated)?;

        self.disc = DiscState::Removing;
        Ok(())
    }

    /// Acknowledge that physical disc removal completed.
    pub fn notify_disc_removed(&mut self) -> Result<(), DiscTransitionError> {
        self.require_lid_open(DiscAction::Removed)?;
        self.require_disc_state(DiscAction::Removed, DiscState::Removing)?;

        self.disc = DiscState::Absent;
        Ok(())
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
                disc_state: None,
                transport_state: None,
            });
        }

        self.lid = next;
        Ok(())
    }

    fn require_lid_open(&self, action: DiscAction) -> Result<(), DiscTransitionError> {
        if self.lid != LidState::Open {
            return Err(self.disc_error(action, DiscTransitionErrorKind::LidNotOpen));
        }

        Ok(())
    }

    fn require_disc_state(
        &self,
        action: DiscAction,
        expected: DiscState,
    ) -> Result<(), DiscTransitionError> {
        if self.disc != expected {
            return Err(self.disc_error(action, DiscTransitionErrorKind::InvalidDiscState));
        }

        Ok(())
    }

    fn disc_error(&self, action: DiscAction, kind: DiscTransitionErrorKind) -> DiscTransitionError {
        DiscTransitionError {
            action,
            lid_state: self.lid,
            disc_state: self.disc,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller() -> De200Controller {
        De200Controller::new(Volume::new(0.5).expect("test volume should be valid"))
    }

    #[test]
    fn open_interlock_allows_stopped_transport() {
        let mut controller = controller();

        controller.request_lid_open().unwrap();

        assert_eq!(controller.lid_state(), LidState::Opening);
        assert_eq!(controller.transport_state(), TransportState::Stopped);
    }

    #[test]
    fn open_interlock_rejects_every_non_stopped_transport_without_mutation() {
        for transport in [
            TransportState::Playing,
            TransportState::Paused,
            TransportState::SeekingForward,
            TransportState::SeekingBackward,
        ] {
            let mut controller = controller();
            controller.transport = transport;

            let error = controller.request_lid_open().unwrap_err();

            assert_eq!(error.action(), LidAction::RequestOpen);
            assert_eq!(error.state(), LidState::Closed);
            assert_eq!(error.disc_state(), None);
            assert_eq!(error.transport_state(), Some(transport));
            assert_eq!(controller.lid_state(), LidState::Closed);
            assert_eq!(controller.transport_state(), transport);
        }
    }
}
