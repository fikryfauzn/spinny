pub mod burn;
pub use burn::{BurnDurability, BurnError, BurnPhase, BurnResult, CleanupFailure, burn};
pub mod add_track;
pub mod appearance;
pub mod audio;
pub mod burned_disc;
pub mod customization;
pub mod draft;
mod draft_migration;
pub mod draft_store;
pub mod error;
pub mod fingerprint;
pub mod format;
#[cfg(target_os = "linux")]
pub mod linux_playback;
pub mod metadata;
pub mod player;
pub mod preflight;
pub mod source;
pub mod source_integrity;
pub mod track;
pub mod track_edit;
pub mod track_import;

pub use add_track::AddTrackRequest;

pub use appearance::{DiscAppearance, DiscColor, DiscImage, DiscImageFormat};

pub use audio::ValidatedLocalAudio;

pub use burned_disc::{BurnedDisc, DiscPayload};

pub use customization::CustomizationSession;

pub use draft::{DRAFT_FORMAT_VERSION, DraftDisc};

pub use draft_store::{load_draft, save_draft};

pub use error::{Result, VdiscError};

pub use fingerprint::SourceFingerprint;

pub use metadata::TrackMetadata;

#[cfg(target_os = "linux")]
pub use linux_playback::{
    CpalBackend, LinuxAudioPlayer, PlaybackBackend, PlaybackBackendEvent, PlaybackError,
    PlaybackPlayer, PlaybackResult, PlaybackSession,
};

pub use player::{Player, PlayerAction, PlayerBoundary, PlayerError, PlayerResult, PlayerState};

pub use preflight::{PreflightIssue, PreflightIssueCode, PreflightReport, run_preflight};

pub use source::{LocalFileSelection, TrackSourceKind, TrackSourceSelection};

pub use source_integrity::verify_draft_source_integrity;

pub use track::DraftTrack;

pub use track_edit::{move_draft_track, remove_draft_track};

pub use track_import::add_validated_local_track;

pub const DISC_TRACK_CAPACITY: usize = 6;
