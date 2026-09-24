pub mod add_track;
pub mod appearance;
pub mod audio;
pub mod customization;
pub mod draft;
pub mod draft_store;
pub mod error;
pub mod fingerprint;
pub mod metadata;
pub mod source;
pub mod source_integrity;
pub mod track;
pub mod track_edit;
pub mod track_import;

pub use add_track::AddTrackRequest;

pub use appearance::{DiscAppearance, DiscColor, DiscImage, DiscImageFormat, DiscSurface};

pub use audio::ValidatedLocalAudio;

pub use customization::CustomizationSession;

pub use draft::{DRAFT_FORMAT_VERSION, DraftDisc};

pub use draft_store::{load_draft, save_draft};

pub use error::{Result, VdiscError};

pub use fingerprint::SourceFingerprint;

pub use metadata::TrackMetadata;

pub use source::{LocalFileSelection, TrackSourceKind, TrackSourceSelection};

pub use source_integrity::verify_draft_source_integrity;

pub use track::DraftTrack;

pub use track_edit::{move_draft_track, remove_draft_track};

pub use track_import::add_validated_local_track;

pub const DISC_TRACK_CAPACITY: usize = 6;
