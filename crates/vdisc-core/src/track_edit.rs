use std::path::Path;

use crate::{DraftTrack, Result, load_draft, save_draft};

pub fn remove_draft_track(draft_path: impl AsRef<Path>, position: usize) -> Result<DraftTrack> {
    let draft_path = draft_path.as_ref();

    let mut draft = load_draft(draft_path)?;

    let removed = draft.remove_track_at(position)?;

    save_draft(draft_path, &draft)?;

    Ok(removed)
}

pub fn move_draft_track(
    draft_path: impl AsRef<Path>,
    from_position: usize,
    to_position: usize,
) -> Result<bool> {
    let draft_path = draft_path.as_ref();

    let mut draft = load_draft(draft_path)?;

    let changed = draft.move_track(from_position, to_position)?;

    /*
     * Don't rewrite the draft for a no-op such as:
     *
     * 3 → 3
     */
    if changed {
        save_draft(draft_path, &draft)?;
    }

    Ok(changed)
}
