use super::*;
use crate::TrackCompletionIntent;
use vdisc_core::PlaybackBackendEvent;

impl<B: PlaybackBackend> AudioPlayerBridge<B> {
    /// Drain only the current session, then perform at most one policy transition.
    /// Deferring completion until after draining lets a queued failure win over EOF.
    pub fn poll(&mut self, controller: &mut De200Controller) -> Result<(), AudioIntegrationError> {
        for _ in 0..16 {
            let Some(event) = self.session.as_mut().and_then(PlaybackSession::poll_event) else {
                break;
            };
            match event {
                PlaybackBackendEvent::TrackFinished => self.pending_eof = true,
                PlaybackBackendEvent::DeviceError(message) => {
                    return Err(self.fail_backend(controller, PlaybackError::Device(message)));
                }
                PlaybackBackendEvent::DecodeError(message) => {
                    return Err(self.fail_backend(controller, PlaybackError::Decode(message)));
                }
            }
        }
        // The first track was verified before seating, but another track
        // may fail the complete background pass. Detect that even when idle,
        // report it exactly once, and retire an active audio session before
        // any further payload can be opened. A later inserted disc resets the
        // latch, so no previous worker can poison the new session.
        if !self.verification_failure_reported {
            let failed = self
                .player
                .disc()
                .and_then(|disc| disc.playback_track_ready(0).err());
            if let Some(error) = failed {
                self.verification_failure_reported = true;
                return Err(self.fail_backend(controller, PlaybackError::Disc(error)));
            }
        }
        if self.pending_eof
            && matches!(
                controller.transport_state(),
                TransportState::Playing
                    | TransportState::SeekingForward
                    | TransportState::SeekingBackward
            )
        {
            self.execute_completion(controller)?;
        }
        Ok(())
    }

    fn execute_completion(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), AudioIntegrationError> {
        self.ensure_transport_sync(controller)?;
        if matches!(
            controller.transport_state(),
            TransportState::SeekingForward | TransportState::SeekingBackward
        ) {
            controller
                .request_scan_end()
                .map_err(CoreIntegrationError::from)?;
        }
        let current = self.current_track_index().ok_or_else(|| {
            self.fail_backend(
                controller,
                PlaybackError::BackendInvariant("EOF without track"),
            )
        })?;
        let count = self.track_count();
        let intent = controller
            .track_completion_intent(current + 1 == count)
            .map_err(CoreIntegrationError::from)?;
        // EOF is already latched in pending_eof. Leave the transport and
        // current session untouched until the next track can be opened.
        // The event loop keeps polling without blocking on verification.
        if matches!(intent, TrackCompletionIntent::Advance)
            && current + 1 < count
            && !self.playback_track_ready(controller, current + 1)?
        {
            return Ok(());
        }
        let result: Result<(), PlaybackError> = (|| {
            match intent {
                TrackCompletionIntent::Advance | TrackCompletionIntent::StopAtDiscEnd => {
                    self.player.track_finished()?
                }
                TrackCompletionIntent::StopAfterCurrent => self.player.stop()?,
                TrackCompletionIntent::RestartDisc => {
                    self.player.track_finished()?;
                    self.player.play()?;
                }
                TrackCompletionIntent::ReplayCurrent => self.player.seek(0)?,
                TrackCompletionIntent::ChooseShuffleTrack => {
                    let target = if count == 1 {
                        0
                    } else {
                        let other = self.rng.usize(0..count - 1);
                        if other >= current { other + 1 } else { other }
                    };
                    for _ in current..target {
                        self.player.next_track()?;
                    }
                    for _ in target..current {
                        self.player.previous()?;
                    }
                    self.player.seek(0)?;
                }
            }
            if self.player.state() == PlayerState::Stopped {
                self.retire_session();
            } else {
                self.replace_session(self.application_gain(controller))?;
            }
            Ok(())
        })();
        self.handle_operation(controller, result)?;
        if matches!(
            intent,
            TrackCompletionIntent::StopAtDiscEnd | TrackCompletionIntent::StopAfterCurrent
        ) {
            controller.commit_backend_stopped_without_resume();
        }
        controller.clear_error_state();
        Ok(())
    }
}
