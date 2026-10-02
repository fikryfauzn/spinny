//! Product-layer appliance behavior for VDISC.
//!
//! This crate models the physical appliance above `vdisc-core`. It is not a
//! second copy of the backend player. Mechanical state and appliance policy
//! belong here; disc validation, payload access, decoding, and audio playback
//! remain backend responsibilities.

mod state;

pub use state::{
    ApplianceErrorState, De200Controller, DiscAction, DiscState, DiscTransitionError,
    DiscTransitionErrorKind, LidAction, LidState, LidTransitionError, PlayMode, PlaybackPosition,
    TransportAction, TransportState, TransportTransitionError, TransportTransitionErrorKind,
    Volume, VolumeError,
};
