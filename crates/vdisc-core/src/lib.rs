pub mod add_track;
pub mod draft;
pub mod draft_store;
pub mod error;

pub use add_track::AddTrackRequest;
pub use draft::{DRAFT_FORMAT_VERSION, DraftDisc};
pub use draft_store::{load_draft, save_draft};
pub use error::{Result, VdiscError};

pub const DISC_TRACK_CAPACITY: usize = 6;
