use crate::{
    DraftTrack, Result, TrackMetadata, ValidatedLocalAudio, VdiscError, load_draft, save_draft,
};

pub fn add_validated_local_track(audio: ValidatedLocalAudio) -> Result<DraftTrack> {
    /*
     * Metadata is extracted from the exact validated
     * audio object here.
     *
     * This prevents callers from accidentally combining
     * metadata from file A with audio from file B.
     */
    let metadata = TrackMetadata::extract(&audio)?;

    let target_path = audio.target_path().to_path_buf();

    let expected_disc_id = audio.target_disc_id();

    let mut draft = load_draft(&target_path)?;

    /*
     * The path may still exist but contain a completely
     * different draft than the one the audio was
     * validated against.
     */
    if draft.id() != expected_disc_id {
        return Err(VdiscError::DraftIdentityMismatch {
            expected: expected_disc_id,
            actual: draft.id(),
        });
    }

    let track = DraftTrack::from_local_audio(&audio, &metadata);

    /*
     * Capacity enforcement lives on DraftDisc itself.
     */
    draft.add_track(track.clone())?;

    /*
     * save_draft() already uses temporary-file +
     * rename semantics.
     */
    save_draft(&target_path, &draft)?;

    Ok(track)
}
