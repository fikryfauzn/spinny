//! Product-layer appliance behavior for VDISC.
//!
//! This crate models the physical appliance above `vdisc-core`. It is not a
//! second copy of the backend player. Mechanical state and appliance policy
//! belong here; disc validation, payload access, decoding, and audio playback
//! remain backend responsibilities.

mod error_translation;
mod lcd;
mod navigation;
mod play_mode;
mod resume;
mod state;
mod volume;

pub use error_translation::{
    DiscFailureClass, PlaybackFailureClass, translate_disc_failure, translate_playback_failure,
};
pub use lcd::{LcdBackendFacts, LcdMessage, LcdPlaybackStatus, LcdSnapshot, LcdTransientMessage};
pub use navigation::{NavigationError, NavigationPlaybackPort};
pub use resume::{ResumePlayError, ResumePlaybackPort};
pub use volume::{AvlsAction, AvlsTransitionError, AvlsTransitionErrorKind};

pub use state::{
    ApplianceErrorState, D_E200_AVLS_VOLUME_CEILING, De200Controller, DiscAction, DiscState,
    DiscTransitionError, DiscTransitionErrorKind, LidAction, LidState, LidTransitionError,
    NavigationAction, NavigationTransitionError, NavigationTransitionErrorKind, PlayMode,
    PlayModeAction, PlayModeTransitionError, PlayModeTransitionErrorKind, PlaybackPosition,
    ScanDirection, TrackCompletionIntent, TransportAction, TransportState,
    TransportTransitionError, TransportTransitionErrorKind, Volume, VolumeError,
};
