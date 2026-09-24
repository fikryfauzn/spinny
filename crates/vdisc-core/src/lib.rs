pub mod add_track;
pub mod audio;
pub mod draft;
pub mod draft_store;
pub mod error;
pub mod source;

pub use add_track::AddTrackRequest;
pub use audio::ValidatedLocalAudio;
pub use draft::{DRAFT_FORMAT_VERSION, DraftDisc};
pub use draft_store::{load_draft, save_draft};
pub use error::{Result, VdiscError};
pub use source::{LocalFileSelection, TrackSourceKind, TrackSourceSelection};

pub const DISC_TRACK_CAPACITY: usize = 6;
