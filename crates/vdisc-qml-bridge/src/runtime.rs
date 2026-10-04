use std::path::PathBuf;

use vdisc_appliance::{
    CorePlayerBridge, De200Controller, DiscState, PlaybackPosition, ScanDirection, Volume,
};

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
pub struct ApplianceRuntime {
    pub(crate) controller: De200Controller,
    pub(crate) backend: CorePlayerBridge,
}

impl ApplianceRuntime {
    pub fn new() -> Self {
        Self {
            controller: De200Controller::new(Volume::new(0.5).expect("valid initial volume")),
            backend: CorePlayerBridge::new(),
        }
    }

    pub fn controller(&self) -> &De200Controller {
        &self.controller
    }

    pub fn backend(&self) -> &CorePlayerBridge {
        &self.backend
    }

    pub fn execute(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Open => self
                .controller
                .request_lid_open()
                .map_err(|e| e.to_string()),
            Command::Close => self
                .controller
                .request_lid_close()
                .map_err(|e| e.to_string()),
            Command::Insert(path) => {
                if path.as_os_str().is_empty() {
                    return Err("disc path is empty".into());
                }
                self.controller
                    .request_disc_insert()
                    .map_err(|e| e.to_string())?;
                let result = self
                    .backend
                    .validate_inserting_disc(&mut self.controller, path)
                    .map_err(|e| e.to_string());
                if result.is_err() && self.controller.disc_state() == DiscState::Inserting {
                    self.controller
                        .notify_disc_validation_rejected()
                        .map_err(|e| e.to_string())?;
                }
                result
            }
            Command::Remove => self
                .controller
                .request_disc_remove()
                .map_err(|e| e.to_string()),
            Command::Play => self
                .backend
                .play(&mut self.controller)
                .map_err(|e| e.to_string()),
            Command::Pause => self
                .backend
                .pause(&mut self.controller)
                .map_err(|e| e.to_string()),
            Command::Stop => self
                .backend
                .stop(&mut self.controller)
                .map_err(|e| e.to_string()),
            Command::Previous => self
                .backend
                .previous(&mut self.controller)
                .map_err(|e| e.to_string()),
            Command::Next => self
                .backend
                .next(&mut self.controller)
                .map_err(|e| e.to_string()),
            Command::ScanBegin(direction) => self
                .controller
                .request_scan_begin(direction)
                .map_err(|e| e.to_string()),
            Command::ScanEnd => self
                .controller
                .request_scan_end()
                .map_err(|e| e.to_string()),
            Command::ScanStep(target_ms) => {
                let target_ms = u64::try_from(target_ms)
                    .map_err(|_| "scan target must be nonnegative".to_owned())?;
                self.backend
                    .scan_seek(
                        &mut self.controller,
                        PlaybackPosition::from_millis(target_ms),
                    )
                    .map_err(|e| e.to_string())
            }
            Command::MenuShort => self
                .controller
                .request_cycle_play_mode()
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Command::MenuLong | Command::ToggleAvls => self
                .controller
                .request_toggle_avls()
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Command::SetHold(enabled) => {
                self.controller.set_hold_enabled(enabled);
                Ok(())
            }
            Command::SetVolume(value) => {
                let volume = Volume::new(value).map_err(|e| e.to_string())?;
                self.controller.request_set_volume(volume);
                Ok(())
            }
            Command::LidOpened => self
                .controller
                .notify_lid_opened()
                .map_err(|e| e.to_string()),
            Command::LidClosed => self
                .controller
                .notify_lid_closed()
                .map_err(|e| e.to_string()),
            Command::DiscInserted => self
                .controller
                .notify_disc_seated()
                .map_err(|e| e.to_string()),
            Command::DiscRemoved => self
                .backend
                .complete_disc_removal(&mut self.controller)
                .map_err(|e| e.to_string()),
        }
    }
}

impl Default for ApplianceRuntime {
    fn default() -> Self {
        Self::new()
    }
}
