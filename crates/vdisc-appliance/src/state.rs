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

impl PlayMode {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Normal => Self::RepeatAll,
            Self::RepeatAll => Self::Single,
            Self::Single => Self::RepeatSingle,
            Self::RepeatSingle => Self::RepeatShuffle,
            Self::RepeatShuffle => Self::Normal,
        }
    }
}

/// Short-MENU and end-of-track policy actions owned by the appliance layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayModeAction {
    Cycle,
    TrackCompleted,
}

/// Backend-neutral consequence selected when the current track completes.
///
/// The controller decides policy only. It does not choose shuffle indexes or
/// perform backend navigation in Objective 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackCompletionIntent {
    Advance,
    StopAtDiscEnd,
    StopAfterCurrent,
    RestartDisc,
    ReplayCurrent,
    ChooseShuffleTrack,
}

/// Why a play-mode request was rejected by the appliance layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayModeTransitionErrorKind {
    HoldEnabled,
    InvalidTransportState,
}

/// A MENU/play-mode action was not legal for the current appliance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayModeTransitionError {
    action: PlayModeAction,
    play_mode: PlayMode,
    transport_state: TransportState,
    kind: PlayModeTransitionErrorKind,
}

impl PlayModeTransitionError {
    pub const fn action(self) -> PlayModeAction {
        self.action
    }

    pub const fn play_mode(self) -> PlayMode {
        self.play_mode
    }

    pub const fn transport_state(self) -> TransportState {
        self.transport_state
    }

    pub const fn kind(self) -> PlayModeTransitionErrorKind {
        self.kind
    }
}

impl fmt::Display for PlayModeTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "play-mode action {:?} is invalid while mode is {:?} and transport is {:?}: {:?}",
            self.action, self.play_mode, self.transport_state, self.kind
        )
    }
}

impl Error for PlayModeTransitionError {}

/// Normalized application playback gain.
///
/// Objective 10 applies AVLS clamping at the appliance-policy layer. Button
/// step count, gain curve, and persistence remain later runtime concerns.
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

/// D-E200 reference-appliance AVLS ceiling used by VDISC.
///
/// The Sony behavioral source establishes that AVLS imposes a deterministic
/// maximum, but does not provide a numeric normalized gain. `0.75` is therefore
/// an explicit VDISC implementation parameter, not a claimed Sony value.
pub const D_E200_AVLS_VOLUME_CEILING: Volume = Volume(0.75);

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
    hold_enabled: bool,
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

    /// Whether HOLD specifically blocked the requested OPEN action.
    pub const fn hold_enabled(self) -> bool {
        self.hold_enabled
    }
}

impl fmt::Display for LidTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hold_enabled {
            write!(
                f,
                "lid action {:?} is blocked because HOLD is enabled",
                self.action
            )
        } else if let Some(transport_state) = self.transport_state {
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

/// Explicit transport inputs owned by the D-E200 appliance controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportAction {
    Play,
    Pause,
    Stop,
}

/// Why a transport action was rejected by the appliance layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportTransitionErrorKind {
    LidNotClosed,
    DiscNotSeated,
    HoldEnabled,
    InvalidTransportState,
    ResumeExecutionRequired,
}

/// A transport action was not legal for the current appliance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportTransitionError {
    action: TransportAction,
    transport_state: TransportState,
    lid_state: LidState,
    disc_state: DiscState,
    hold_enabled: bool,
    kind: TransportTransitionErrorKind,
}

impl TransportTransitionError {
    pub const fn action(self) -> TransportAction {
        self.action
    }

    pub const fn transport_state(self) -> TransportState {
        self.transport_state
    }

    pub const fn lid_state(self) -> LidState {
        self.lid_state
    }

    pub const fn disc_state(self) -> DiscState {
        self.disc_state
    }

    pub const fn hold_enabled(self) -> bool {
        self.hold_enabled
    }

    pub const fn kind(self) -> TransportTransitionErrorKind {
        self.kind
    }
}

impl fmt::Display for TransportTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "transport action {:?} is invalid while transport is {:?}, lid is {:?}, disc is {:?}, hold is {}: {:?}",
            self.action,
            self.transport_state,
            self.lid_state,
            self.disc_state,
            self.hold_enabled,
            self.kind
        )
    }
}

impl Error for TransportTransitionError {}

/// Direction of a held D-E200 AMS control during fast scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDirection {
    Forward,
    Backward,
}

/// Navigation actions exposed by the physical previous/next controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    Previous,
    Next,
    BeginScan(ScanDirection),
    ScanSeek,
    EndScan,
}

/// Why a navigation request was rejected by the appliance layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTransitionErrorKind {
    HoldEnabled,
    LidNotClosed,
    DiscNotSeated,
    InvalidTransportState,
    WrongScanDirection,
}

/// A previous/next/scan action was not legal for the current appliance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavigationTransitionError {
    action: NavigationAction,
    transport_state: TransportState,
    lid_state: LidState,
    disc_state: DiscState,
    kind: NavigationTransitionErrorKind,
}

impl NavigationTransitionError {
    pub const fn action(self) -> NavigationAction {
        self.action
    }

    pub const fn transport_state(self) -> TransportState {
        self.transport_state
    }

    pub const fn lid_state(self) -> LidState {
        self.lid_state
    }

    pub const fn disc_state(self) -> DiscState {
        self.disc_state
    }

    pub const fn kind(self) -> NavigationTransitionErrorKind {
        self.kind
    }
}

impl fmt::Display for NavigationTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "navigation action {:?} is invalid while transport is {:?}, lid is {:?}, disc is {:?}: {:?}",
            self.action, self.transport_state, self.lid_state, self.disc_state, self.kind
        )
    }
}

impl Error for NavigationTransitionError {}

/// Internal marker returned by the centralized HOLD gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HoldLockError;

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

    /// Set the physical HOLD switch position.
    ///
    /// HOLD itself remains operable while the controls are locked so the user
    /// can always disable it again. Changing the switch never rewrites
    /// transport, mechanical, resume, or playback-mode state.
    pub fn set_hold_enabled(&mut self, enabled: bool) {
        self.hold_enabled = enabled;
    }

    /// Central gate for physical controls affected by HOLD.
    ///
    /// Individual command families translate this marker into their existing
    /// typed error surface. Completion events and automatic playback events do
    /// not pass through this gate.
    pub(crate) fn ensure_controls_unlocked(&self) -> Result<(), HoldLockError> {
        if self.hold_enabled {
            Err(HoldLockError)
        } else {
            Ok(())
        }
    }

    pub const fn avls_enabled(&self) -> bool {
        self.avls_enabled
    }

    pub const fn volume(&self) -> Volume {
        self.volume
    }

    pub(crate) fn commit_volume(&mut self, volume: Volume) {
        self.volume = volume;
    }

    pub(crate) fn commit_avls_enabled(&mut self, enabled: bool) {
        self.avls_enabled = enabled;
    }

    pub const fn resume_position(&self) -> Option<PlaybackPosition> {
        self.resume_position
    }

    pub const fn error_state(&self) -> Option<ApplianceErrorState> {
        self.error
    }

    /// Begin held fast scan while actively playing.
    ///
    /// The controller owns only the physical hold-state choreography. Actual
    /// seek targets are executed through the backend-neutral navigation port.
    pub fn request_scan_begin(
        &mut self,
        direction: ScanDirection,
    ) -> Result<(), NavigationTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.navigation_error(
                NavigationAction::BeginScan(direction),
                NavigationTransitionErrorKind::HoldEnabled,
            ));
        }

        self.validate_navigation_mechanics(NavigationAction::BeginScan(direction))?;

        if self.transport != TransportState::Playing {
            return Err(self.navigation_error(
                NavigationAction::BeginScan(direction),
                NavigationTransitionErrorKind::InvalidTransportState,
            ));
        }

        self.transport = match direction {
            ScanDirection::Forward => TransportState::SeekingForward,
            ScanDirection::Backward => TransportState::SeekingBackward,
        };
        Ok(())
    }

    /// End held fast scan and return to the playing state it originated from.
    pub fn request_scan_end(&mut self) -> Result<(), NavigationTransitionError> {
        self.validate_navigation_mechanics(NavigationAction::EndScan)?;

        if !matches!(
            self.transport,
            TransportState::SeekingForward | TransportState::SeekingBackward
        ) {
            return Err(self.navigation_error(
                NavigationAction::EndScan,
                NavigationTransitionErrorKind::InvalidTransportState,
            ));
        }

        self.transport = TransportState::Playing;
        Ok(())
    }

    pub(crate) fn validate_ams_request(
        &self,
        action: NavigationAction,
    ) -> Result<(), NavigationTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.navigation_error(action, NavigationTransitionErrorKind::HoldEnabled));
        }

        self.validate_navigation_mechanics(action)?;

        if !matches!(
            self.transport,
            TransportState::Stopped | TransportState::Playing | TransportState::Paused
        ) {
            return Err(
                self.navigation_error(action, NavigationTransitionErrorKind::InvalidTransportState)
            );
        }

        Ok(())
    }

    pub(crate) fn validate_scan_seek_request(
        &self,
        current: PlaybackPosition,
        target: PlaybackPosition,
    ) -> Result<(), NavigationTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.navigation_error(
                NavigationAction::ScanSeek,
                NavigationTransitionErrorKind::HoldEnabled,
            ));
        }

        self.validate_navigation_mechanics(NavigationAction::ScanSeek)?;

        let correct_direction = match self.transport {
            TransportState::SeekingForward => target >= current,
            TransportState::SeekingBackward => target <= current,
            _ => {
                return Err(self.navigation_error(
                    NavigationAction::ScanSeek,
                    NavigationTransitionErrorKind::InvalidTransportState,
                ));
            }
        };

        if !correct_direction {
            return Err(self.navigation_error(
                NavigationAction::ScanSeek,
                NavigationTransitionErrorKind::WrongScanDirection,
            ));
        }

        Ok(())
    }

    pub(crate) fn validate_play_mode_action(
        &self,
        action: PlayModeAction,
    ) -> Result<(), PlayModeTransitionError> {
        if matches!(action, PlayModeAction::Cycle) && self.ensure_controls_unlocked().is_err() {
            return Err(PlayModeTransitionError {
                action,
                play_mode: self.play_mode,
                transport_state: self.transport,
                kind: PlayModeTransitionErrorKind::HoldEnabled,
            });
        }

        if self.transport != TransportState::Playing {
            return Err(PlayModeTransitionError {
                action,
                play_mode: self.play_mode,
                transport_state: self.transport,
                kind: PlayModeTransitionErrorKind::InvalidTransportState,
            });
        }

        Ok(())
    }

    pub(crate) fn cycle_play_mode(&mut self) -> PlayMode {
        self.play_mode = self.play_mode.next();
        self.play_mode
    }

    pub(crate) fn clear_resume_position(&mut self) {
        self.resume_position = None;
    }

    fn validate_navigation_mechanics(
        &self,
        action: NavigationAction,
    ) -> Result<(), NavigationTransitionError> {
        if self.lid != LidState::Closed {
            return Err(self.navigation_error(action, NavigationTransitionErrorKind::LidNotClosed));
        }

        if self.disc != DiscState::Seated {
            return Err(self.navigation_error(action, NavigationTransitionErrorKind::DiscNotSeated));
        }

        Ok(())
    }

    /// Start playback from Stopped when no resume memory is pending.
    ///
    /// Once STOP has captured a resume position, callers must use the
    /// resume-aware adapter in `crate::resume`. This prevents product code from
    /// silently bypassing the required seek-before-play sequence.
    pub fn request_play(&mut self) -> Result<(), TransportTransitionError> {
        self.validate_play_request()?;

        if self.resume_position.is_some() {
            return Err(self.transport_error(
                TransportAction::Play,
                TransportTransitionErrorKind::ResumeExecutionRequired,
            ));
        }

        self.commit_playing();
        Ok(())
    }

    pub(crate) fn validate_play_request(&self) -> Result<(), TransportTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.transport_error(
                TransportAction::Play,
                TransportTransitionErrorKind::HoldEnabled,
            ));
        }

        if self.lid != LidState::Closed {
            return Err(self.transport_error(
                TransportAction::Play,
                TransportTransitionErrorKind::LidNotClosed,
            ));
        }

        if self.disc != DiscState::Seated {
            return Err(self.transport_error(
                TransportAction::Play,
                TransportTransitionErrorKind::DiscNotSeated,
            ));
        }

        if self.transport != TransportState::Stopped {
            return Err(self.transport_error(
                TransportAction::Play,
                TransportTransitionErrorKind::InvalidTransportState,
            ));
        }

        Ok(())
    }

    pub(crate) fn commit_playing(&mut self) {
        debug_assert_eq!(self.transport, TransportState::Stopped);
        self.transport = TransportState::Playing;
    }

    /// Toggle the dedicated VDISC pause control.
    ///
    /// Playing becomes Paused and Paused becomes Playing. Other transport
    /// states are rejected. HOLD blocks the pause control as specified by the
    /// Phase 2 appliance contract.
    pub fn request_pause(&mut self) -> Result<(), TransportTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.transport_error(
                TransportAction::Pause,
                TransportTransitionErrorKind::HoldEnabled,
            ));
        }

        self.transport = match self.transport {
            TransportState::Playing => TransportState::Paused,
            TransportState::Paused => TransportState::Playing,
            _ => {
                return Err(self.transport_error(
                    TransportAction::Pause,
                    TransportTransitionErrorKind::InvalidTransportState,
                ));
            }
        };

        Ok(())
    }

    /// Stop transport and capture appliance-level resume memory.
    ///
    /// Live playback position remains backend-owned, so the caller supplies the
    /// current position at the adapter boundary. Objective 5 stores that value
    /// before committing Stopped state. Objective 6 will consume it when PLAY
    /// resumes from Stopped.
    pub fn request_stop(
        &mut self,
        current_position: PlaybackPosition,
    ) -> Result<(), TransportTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(self.transport_error(
                TransportAction::Stop,
                TransportTransitionErrorKind::HoldEnabled,
            ));
        }

        if !matches!(
            self.transport,
            TransportState::Playing
                | TransportState::Paused
                | TransportState::SeekingForward
                | TransportState::SeekingBackward
        ) {
            return Err(self.transport_error(
                TransportAction::Stop,
                TransportTransitionErrorKind::InvalidTransportState,
            ));
        }

        self.resume_position = Some(current_position);
        self.transport = TransportState::Stopped;
        Ok(())
    }

    /// Begin opening the lid.
    ///
    /// OPEN is permitted only while transport is fully stopped. The interlock
    /// is enforced before any mechanical motion begins so rejected requests do
    /// not partially mutate appliance state.
    pub fn request_lid_open(&mut self) -> Result<(), LidTransitionError> {
        if self.ensure_controls_unlocked().is_err() {
            return Err(LidTransitionError {
                action: LidAction::RequestOpen,
                state: self.lid,
                disc_state: None,
                transport_state: None,
                hold_enabled: true,
            });
        }

        if self.lid != LidState::Closed {
            return Err(LidTransitionError {
                action: LidAction::RequestOpen,
                state: self.lid,
                disc_state: None,
                transport_state: None,
                hold_enabled: false,
            });
        }

        if self.transport != TransportState::Stopped {
            return Err(LidTransitionError {
                action: LidAction::RequestOpen,
                state: self.lid,
                disc_state: None,
                transport_state: Some(self.transport),
                hold_enabled: false,
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
                hold_enabled: false,
            });
        }

        if !matches!(self.disc, DiscState::Absent | DiscState::Seated) {
            return Err(LidTransitionError {
                action: LidAction::RequestClose,
                state: self.lid,
                disc_state: Some(self.disc),
                transport_state: None,
                hold_enabled: false,
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
        self.resume_position = None;
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
                hold_enabled: false,
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

    fn transport_error(
        &self,
        action: TransportAction,
        kind: TransportTransitionErrorKind,
    ) -> TransportTransitionError {
        TransportTransitionError {
            action,
            transport_state: self.transport,
            lid_state: self.lid,
            disc_state: self.disc,
            hold_enabled: self.hold_enabled,
            kind,
        }
    }

    fn navigation_error(
        &self,
        action: NavigationAction,
        kind: NavigationTransitionErrorKind,
    ) -> NavigationTransitionError {
        NavigationTransitionError {
            action,
            transport_state: self.transport,
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

    #[test]
    fn transport_hold_blocks_play_and_pause_without_mutation() {
        let mut controller = controller();
        controller.lid = LidState::Closed;
        controller.disc = DiscState::Seated;
        controller.hold_enabled = true;

        let play_error = controller.request_play().unwrap_err();
        assert_eq!(play_error.kind(), TransportTransitionErrorKind::HoldEnabled);
        assert_eq!(controller.transport_state(), TransportState::Stopped);

        controller.hold_enabled = false;
        controller.request_play().unwrap();
        controller.hold_enabled = true;

        let pause_error = controller.request_pause().unwrap_err();
        assert_eq!(
            pause_error.kind(),
            TransportTransitionErrorKind::HoldEnabled
        );
        assert_eq!(controller.transport_state(), TransportState::Playing);
    }

    #[test]
    fn transport_stop_accepts_both_seeking_states_and_captures_position() {
        for transport in [
            TransportState::SeekingForward,
            TransportState::SeekingBackward,
        ] {
            let mut controller = controller();
            controller.transport = transport;
            let position = PlaybackPosition::from_millis(42_500);

            controller.request_stop(position).unwrap();

            assert_eq!(controller.transport_state(), TransportState::Stopped);
            assert_eq!(controller.resume_position(), Some(position));
        }
    }
}
