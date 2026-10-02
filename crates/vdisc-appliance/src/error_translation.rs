use crate::ApplianceErrorState;

/// Backend-neutral classification for failures encountered while opening,
/// validating, or reading a `.vdisc`.
///
/// This intentionally does not mirror `vdisc_core::FormatErrorKind` one-for-one.
/// In Campaign 01, `ResourceLimit` is used for both invalid-format policy limits
/// and runtime resource inability. Objective 13 must classify that concrete
/// backend context before crossing this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscFailureClass {
    /// The selected bytes do not represent a supported valid VDISC.
    ///
    /// Examples: malformed/unsupported archive structure, invalid schema,
    /// integrity mismatch, invalid artwork/media, or a product-format limit
    /// violation.
    InvalidContent,

    /// The selected disc could not be read from its backing storage.
    ReadFailure,

    /// The environment could not supply resources required to inspect/read an
    /// otherwise not-yet-proven-invalid disc.
    ResourceFailure,
}

/// Backend-neutral classification for failures encountered while operating the
/// loaded player.
///
/// Objective 13 maps concrete `vdisc-core` player/playback errors into this
/// semantic taxonomy. Keeping this module independent of `vdisc-core` preserves
/// the Phase 2 dependency direction until the real integration objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackFailureClass {
    /// A disc/read failure surfaced while playback was accessing disc content.
    Disc(DiscFailureClass),

    /// A legal appliance action could not be completed by the backend transport
    /// layer. Normal user-facing track-boundary policy should be resolved before
    /// it reaches this failure class.
    TransportFailure,

    /// Media decoding failed after the disc entered the playable session.
    DecodeFailure,

    /// The backend detected an internal state/coordination invariant failure.
    BackendInvariant,

    /// No usable Linux audio output device exists.
    NoOutputDevice,

    /// An output device exists but cannot satisfy the required PCM output.
    UnsupportedOutput,

    /// Runtime audio-device/stream operation failed.
    DeviceFailure,
}

/// Translate one disc failure into the constrained appliance-facing state.
pub const fn translate_disc_failure(failure: DiscFailureClass) -> ApplianceErrorState {
    match failure {
        DiscFailureClass::InvalidContent => ApplianceErrorState::InvalidDisc,
        DiscFailureClass::ReadFailure | DiscFailureClass::ResourceFailure => {
            ApplianceErrorState::UnreadableDisc
        }
    }
}

/// Translate one playback failure into the constrained appliance-facing state.
pub const fn translate_playback_failure(failure: PlaybackFailureClass) -> ApplianceErrorState {
    match failure {
        PlaybackFailureClass::Disc(failure) => translate_disc_failure(failure),
        PlaybackFailureClass::TransportFailure
        | PlaybackFailureClass::DecodeFailure
        | PlaybackFailureClass::BackendInvariant => ApplianceErrorState::PlaybackFailure,
        PlaybackFailureClass::NoOutputDevice
        | PlaybackFailureClass::UnsupportedOutput
        | PlaybackFailureClass::DeviceFailure => ApplianceErrorState::AudioOutputFailure,
    }
}
