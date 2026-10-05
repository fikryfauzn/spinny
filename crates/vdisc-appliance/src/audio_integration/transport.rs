use super::*;
use crate::{NavigationAction, NavigationPlaybackPort, ResumePlaybackPort};

impl<B: PlaybackBackend> AudioPlayerBridge<B> {
    pub fn play(&mut self, controller: &mut De200Controller) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        controller
            .validate_play_request()
            .map_err(CoreIntegrationError::from)?;
        self.ensure_transport_sync(controller)?;
        let gain = self.application_gain(controller);
        let result = controller.request_play_with(&mut AudioPort { bridge: self, gain });
        match result {
            Ok(()) => {
                controller.clear_error_state();
                Ok(())
            }
            Err(ResumePlayError::Backend(error)) => Err(self.fail_backend(controller, error)),
            Err(error) => Err(AudioIntegrationError::Resume(error)),
        }
    }
    pub fn pause(&mut self, controller: &mut De200Controller) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        controller
            .validate_pause_request()
            .map_err(CoreIntegrationError::from)?;
        self.ensure_transport_sync(controller)?;
        let session = self
            .session
            .as_ref()
            .expect("transport sync requires session");
        let result = if controller.transport_state() == TransportState::Playing {
            session
                .pause()
                .and_then(|()| self.player.pause().map_err(Into::into))
        } else {
            session
                .set_gain(self.application_gain(controller))
                .and_then(|()| session.play())
                .and_then(|()| self.player.play().map_err(Into::into))
        };
        self.handle_operation(controller, result)?;
        controller.commit_pause_toggle();
        controller.clear_error_state();
        Ok(())
    }
    pub fn stop(&mut self, controller: &mut De200Controller) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        controller
            .validate_stop_request()
            .map_err(CoreIntegrationError::from)?;
        self.ensure_transport_sync(controller)?;
        let position = self.position();
        self.retire_session();
        let result = self.player.stop().map_err(Into::into);
        self.handle_operation(controller, result)?;
        controller.commit_stopped_with_resume(position);
        controller.clear_error_state();
        Ok(())
    }
    pub fn next(&mut self, controller: &mut De200Controller) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        controller
            .validate_ams_request(NavigationAction::Next)
            .map_err(CoreIntegrationError::from)?;
        self.ensure_transport_sync(controller)?;
        let gain = self.application_gain(controller);
        let result = controller.request_next_with(&mut AudioPort { bridge: self, gain });
        self.finish_navigation(controller, result)
    }
    pub fn previous(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        controller
            .validate_ams_request(NavigationAction::Previous)
            .map_err(CoreIntegrationError::from)?;
        self.ensure_transport_sync(controller)?;
        let position = if controller.transport_state() == TransportState::Stopped {
            controller
                .resume_position()
                .unwrap_or_else(|| self.position())
        } else {
            self.position()
        };
        let gain = self.application_gain(controller);
        let result =
            controller.request_previous_with(&mut AudioPort { bridge: self, gain }, position);
        self.finish_navigation(controller, result)
    }
    pub fn scan_seek(
        &mut self,
        controller: &mut De200Controller,
        target: PlaybackPosition,
    ) -> Result<(), AudioIntegrationError> {
        self.poll(controller)?;
        self.ensure_transport_sync(controller)?;
        let position = self.position();
        let gain = self.application_gain(controller);
        let result = controller.request_scan_seek_with(
            &mut AudioPort { bridge: self, gain },
            position,
            target,
        );
        self.finish_navigation(controller, result)
    }
    pub fn sync_gain(
        &mut self,
        controller: &mut De200Controller,
    ) -> Result<(), AudioIntegrationError> {
        let result = self.session.as_ref().map_or(Ok(()), |session| {
            session.set_gain(self.application_gain(controller))
        });
        self.handle_operation(controller, result)
    }
    fn finish_navigation(
        &mut self,
        controller: &mut De200Controller,
        result: Result<(), NavigationError<PlaybackError>>,
    ) -> Result<(), AudioIntegrationError> {
        match result {
            Ok(()) => {
                if self.session.is_some() {
                    controller.clear_error_state();
                }
                Ok(())
            }
            Err(NavigationError::Backend(error))
                if !matches!(
                    error,
                    PlaybackError::Player(
                        PlayerError::Boundary { .. } | PlayerError::SeekOutOfRange { .. }
                    )
                ) =>
            {
                Err(self.fail_backend(controller, error))
            }
            Err(error) => Err(AudioIntegrationError::Navigation(error)),
        }
    }
}

struct AudioPort<'a, B: PlaybackBackend> {
    bridge: &'a mut AudioPlayerBridge<B>,
    gain: f32,
}
impl<B: PlaybackBackend> AudioPort<'_, B> {
    fn seek_player(&mut self, position: PlaybackPosition) -> Result<(), PlaybackError> {
        if self.bridge.player.state() == PlayerState::Stopped {
            self.bridge
                .player
                .restore_stopped_position(position.as_millis())?;
        } else {
            self.bridge.player.seek(position.as_millis())?;
        }
        Ok(())
    }
    fn replace_if_active(&mut self) -> Result<(), PlaybackError> {
        if self.bridge.player.state() != PlayerState::Stopped {
            self.bridge.replace_session(self.gain)?;
        }
        Ok(())
    }
}
impl<B: PlaybackBackend> ResumePlaybackPort for AudioPort<'_, B> {
    type Error = PlaybackError;
    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.seek_player(position)
    }
    fn play(&mut self) -> Result<(), Self::Error> {
        self.bridge.player.play()?;
        self.bridge.replace_session(self.gain)
    }
}
impl<B: PlaybackBackend> NavigationPlaybackPort for AudioPort<'_, B> {
    type Error = PlaybackError;
    fn seek(&mut self, position: PlaybackPosition) -> Result<(), Self::Error> {
        self.seek_player(position)?;
        self.replace_if_active()
    }
    fn next_track(&mut self) -> Result<(), Self::Error> {
        self.bridge.player.next_track()?;
        self.replace_if_active()
    }
    fn previous_track(&mut self) -> Result<(), Self::Error> {
        self.bridge.player.previous()?;
        self.replace_if_active()
    }
}
