use crate::state::{
    De200Controller, PlayMode, PlayModeAction, PlayModeTransitionError, TrackCompletionIntent,
};

impl De200Controller {
    /// Handle a short MENU press while playback is active.
    ///
    /// Objective 9 routes this physical control through the centralized HOLD
    /// gate. Objective 10 handles long MENU separately as the AVLS toggle.
    pub fn request_cycle_play_mode(&mut self) -> Result<PlayMode, PlayModeTransitionError> {
        self.validate_play_mode_action(PlayModeAction::Cycle)?;
        Ok(self.cycle_play_mode())
    }

    /// Resolve the appliance policy for an end-of-track event.
    ///
    /// This method is intentionally side-effect free. It does not navigate the
    /// backend, stop transport, choose a shuffle index, or mutate resume state.
    /// The later runtime/backend adapter executes the returned intent and may
    /// commit resulting transport state only after the backend succeeds.
    pub fn track_completion_intent(
        &self,
        is_last_track: bool,
    ) -> Result<TrackCompletionIntent, PlayModeTransitionError> {
        self.validate_play_mode_action(PlayModeAction::TrackCompleted)?;

        let intent = match self.play_mode() {
            PlayMode::Normal if is_last_track => TrackCompletionIntent::StopAtDiscEnd,
            PlayMode::Normal => TrackCompletionIntent::Advance,
            PlayMode::RepeatAll if is_last_track => TrackCompletionIntent::RestartDisc,
            PlayMode::RepeatAll => TrackCompletionIntent::Advance,
            PlayMode::Single => TrackCompletionIntent::StopAfterCurrent,
            PlayMode::RepeatSingle => TrackCompletionIntent::ReplayCurrent,
            PlayMode::RepeatShuffle => TrackCompletionIntent::ChooseShuffleTrack,
        };

        Ok(intent)
    }
}
