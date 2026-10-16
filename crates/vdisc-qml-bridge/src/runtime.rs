use crate::lcd_feedback::LcdFeedback;
use std::path::PathBuf;
use std::{
    error::Error,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Instant,
};

use vdisc_appliance::{
    AudioPlayerBridge, De200Controller, DiscFailureClass, DiscState, PlaybackPosition,
    ScanDirection, Volume, classify_format_error,
};
use vdisc_core::{BurnedDisc, CpalBackend, PlaybackBackend, format::FormatError};

#[derive(Debug)]
pub enum Command {
    Open,
    Close,
    Insert(PathBuf),
    Remove,
    Play,
    Pause,
    Stop,
    Previous,
    Next,
    ScanBegin(ScanDirection),
    ScanEnd,
    ScanStep(i64),
    ScanRelative(i64),
    MenuShort,
    MenuLong,
    ToggleAvls,
    SetHold(bool),
    SetVolume(f32),
    LidOpened,
    LidClosed,
    DiscInserted,
    DiscRemoved,
}

/// Thin command boundary over the two authoritative Phase 2 owners.
pub struct ApplianceRuntime<B: PlaybackBackend = CpalBackend> {
    pub(crate) controller: De200Controller,
    pub(crate) backend: AudioPlayerBridge<B>,
    pub(crate) feedback: LcdFeedback,
    pending_validation: Option<Receiver<Result<BurnedDisc, FormatError>>>,
    insertion_motion_complete: bool,
    validation_started_at: Option<Instant>,
}

impl ApplianceRuntime<CpalBackend> {
    pub fn new() -> Self {
        Self::with_backend(CpalBackend)
    }
}

impl<B: PlaybackBackend> ApplianceRuntime<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            controller: De200Controller::new(Volume::new(0.5).expect("valid initial volume")),
            backend: AudioPlayerBridge::with_backend(backend),
            feedback: LcdFeedback::default(),
            pending_validation: None,
            insertion_motion_complete: false,
            validation_started_at: None,
        }
    }

    /// True while a single background validator still owns the insertion.
    pub fn insertion_pending(&self) -> bool {
        self.pending_validation.is_some()
    }

    /// Collect the worker result on the Qt-owning thread, never on the worker.
    /// None means the worker is still running or no insertion is active.
    pub fn poll_insertion(&mut self) -> Option<Result<(), String>> {
        let result = match self.pending_validation.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                self.pending_validation = None;
                self.validation_started_at = None;
                let _ = self
                    .controller
                    .notify_disc_validation_failed(DiscFailureClass::ResourceFailure);
                self.insertion_motion_complete = false;
                return Some(Err("disc validation worker terminated unexpectedly".into()));
            }
        };
        self.pending_validation = None;
        if let Some(start) = self.validation_started_at.take()
            && std::env::var_os("VDISC_TIMING").is_some()
        {
            eprintln!(
                "vdisc: first-track verified mount took {} ms",
                start.elapsed().as_millis()
            );
        }
        let outcome = match result {
            Ok(disc) => {
                let accepted = self
                    .backend
                    .accept_validated_disc(&mut self.controller, disc)
                    .map_err(|error| error.to_string());
                if accepted.is_ok() && self.insertion_motion_complete {
                    self.controller
                        .notify_disc_seated()
                        .map_err(|error| error.to_string())
                } else {
                    accepted
                }
            }
            Err(error) => {
                let result = self
                    .controller
                    .notify_disc_validation_failed(classify_format_error(&error))
                    .map_err(|transition| transition.to_string());
                result.and(Err(format!("disc verification failed: {error}")))
            }
        };
        if outcome.is_err() {
            // Fail closed if an adapter error left the handshake unfinished.
            if self.controller.disc_state() == DiscState::Inserting {
                let _ = self.controller.notify_disc_validation_rejected();
            }
            self.insertion_motion_complete = false;
        }
        Some(outcome)
    }

    pub fn controller(&self) -> &De200Controller {
        &self.controller
    }

    pub fn backend(&self) -> &AudioPlayerBridge<B> {
        &self.backend
    }

    pub fn execute(&mut self, command: Command) -> Result<(), String> {
        self.execute_at(command, Instant::now())
    }

    pub(crate) fn execute_at(&mut self, command: Command, now: Instant) -> Result<(), String> {
        self.feedback.expire(now);
        self.poll_backend()?;
        match command {
            Command::Open => self
                .controller
                .request_lid_open()
                .map_err(|e| self.reject(e, now)),
            Command::Close => self
                .controller
                .request_lid_close()
                .map_err(|e| self.reject(e, now)),
            Command::Insert(path) => {
                if path.as_os_str().is_empty() {
                    return Err("disc path is empty".into());
                }
                self.controller
                    .request_disc_insert()
                    .map_err(|e| self.reject(e, now))?;
                let (sender, receiver) = mpsc::channel();
                // One insertion is permitted by the controller's Absent guard.
                // The worker only produces a verified handle; it never mutates Qt.
                let spawned = std::thread::Builder::new()
                    .name("vdisc-disc-validation".into())
                    .spawn(move || {
                        let result = BurnedDisc::open_for_playback(&path);
                        let _ = sender.send(result);
                    });
                if let Err(error) = spawned {
                    let _ = self
                        .controller
                        .notify_disc_validation_failed(DiscFailureClass::ResourceFailure);
                    return Err(format!("could not start disc validation: {error}"));
                }
                self.pending_validation = Some(receiver);
                self.insertion_motion_complete = false;
                self.validation_started_at = Some(Instant::now());
                Ok(())
            }
            Command::Remove => self
                .controller
                .request_disc_remove()
                .map_err(|e| self.reject(e, now)),
            Command::Play => self
                .backend
                .play(&mut self.controller)
                .map_err(|e| self.reject(e, now)),
            Command::Pause => self
                .backend
                .pause(&mut self.controller)
                .map_err(|e| self.reject(e, now)),
            Command::Stop => self
                .backend
                .stop(&mut self.controller)
                .map_err(|e| self.reject(e, now)),
            Command::Previous => self
                .backend
                .previous(&mut self.controller)
                .map_err(|e| self.reject(e, now)),
            Command::Next => self
                .backend
                .next(&mut self.controller)
                .map_err(|e| self.reject(e, now)),
            Command::ScanBegin(direction) => self
                .controller
                .request_scan_begin(direction)
                .map_err(|e| self.reject(e, now)),
            Command::ScanEnd => self
                .controller
                .request_scan_end()
                .map_err(|e| self.reject(e, now)),
            Command::ScanStep(target_ms) => {
                let target_ms = u64::try_from(target_ms)
                    .map_err(|_| "scan target must be nonnegative".to_owned())?;
                self.backend
                    .scan_seek(
                        &mut self.controller,
                        PlaybackPosition::from_millis(target_ms),
                    )
                    .map_err(|e| self.reject(e, now))
            }
            Command::ScanRelative(delta_ms) => {
                use vdisc_appliance::TransportState;
                match (self.controller.transport_state(), delta_ms) {
                    (TransportState::SeekingForward, 1..)
                    | (TransportState::SeekingBackward, ..=-1) => (),
                    _ => {
                        return Err(
                            "relative scan requires active scan in the requested direction".into(),
                        );
                    }
                }
                let target = crate::scan_target::relative_target(
                    self.backend.position().as_millis(),
                    self.backend.current_track_duration_ms(),
                    delta_ms,
                )
                .map_err(str::to_owned)?;
                self.backend
                    .scan_seek(&mut self.controller, PlaybackPosition::from_millis(target))
                    .map_err(|e| self.reject(e, now))
            }
            Command::MenuShort => self
                .controller
                .request_cycle_play_mode()
                .map(|_| ())
                .map_err(|e| self.reject(e, now)),
            Command::MenuLong | Command::ToggleAvls => {
                self.controller
                    .request_toggle_avls()
                    .map_err(|e| self.reject(e, now))?;
                self.backend
                    .sync_gain(&mut self.controller)
                    .map_err(|e| e.to_string())
            }
            Command::SetHold(enabled) => {
                self.controller.set_hold_enabled(enabled);
                if !enabled {
                    self.feedback.clear();
                }
                Ok(())
            }
            Command::SetVolume(value) => {
                let volume = Volume::new(value).map_err(|e| e.to_string())?;
                self.controller.request_set_volume(volume);
                self.backend
                    .sync_gain(&mut self.controller)
                    .map_err(|e| e.to_string())
            }
            Command::LidOpened => self
                .controller
                .notify_lid_opened()
                .map_err(|e| e.to_string()),
            Command::LidClosed => self
                .controller
                .notify_lid_closed()
                .map_err(|e| e.to_string()),
            Command::DiscInserted => {
                if self.controller.disc_state() != DiscState::Inserting
                    || self.insertion_motion_complete
                {
                    return Err("disc insertion animation is not pending".into());
                }
                self.insertion_motion_complete = true;
                // Visual completion can precede validation. Seat only when both
                // have completed, retaining the original appliance invariant.
                if self.pending_validation.is_some() {
                    Ok(())
                } else {
                    self.controller
                        .notify_disc_seated()
                        .map_err(|e| e.to_string())
                }
            }
            Command::DiscRemoved => self
                .backend
                .complete_disc_removal(&mut self.controller)
                .map_err(|e| e.to_string()),
        }
    }

    pub fn poll_backend(&mut self) -> Result<(), String> {
        self.backend
            .poll(&mut self.controller)
            .map_err(|e| e.to_string())
    }

    pub fn refresh_lcd_feedback(&mut self) {
        self.feedback.expire(Instant::now());
    }

    fn reject<E: Error + 'static>(&mut self, error: E, now: Instant) -> String {
        self.feedback.record_rejection(&error, now);
        error.to_string()
    }
}

impl Default for ApplianceRuntime {
    fn default() -> Self {
        Self::new()
    }
}
