use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::{ApplianceRuntime, ApplianceSnapshot, Command};
use vdisc_appliance::ScanDirection;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, lid_state, READ, NOTIFY, cxx_name = "lidState")]
        #[qproperty(QString, disc_state, READ, NOTIFY, cxx_name = "discState")]
        #[qproperty(QString, transport_state, READ, NOTIFY, cxx_name = "transportState")]
        #[qproperty(QString, play_mode, READ, NOTIFY, cxx_name = "playMode")]
        #[qproperty(bool, hold_enabled, READ, NOTIFY, cxx_name = "holdEnabled")]
        #[qproperty(bool, avls_enabled, READ, NOTIFY, cxx_name = "avlsEnabled")]
        #[qproperty(f32, volume, READ, NOTIFY)]
        #[qproperty(f32, application_gain, READ, NOTIFY, cxx_name = "applicationGain")]
        #[qproperty(QString, machine_error, READ, NOTIFY, cxx_name = "machineError")]
        #[qproperty(i32, lcd_track_number, READ, NOTIFY, cxx_name = "lcdTrackNumber")]
        #[qproperty(i64, lcd_elapsed_ms, READ, NOTIFY, cxx_name = "lcdElapsedMs")]
        #[qproperty(i32, lcd_total_tracks, READ, NOTIFY, cxx_name = "lcdTotalTracks")]
        #[qproperty(i64, lcd_total_ms, READ, NOTIFY, cxx_name = "lcdTotalMs")]
        #[qproperty(QString, lcd_play_mode, READ, NOTIFY, cxx_name = "lcdPlayMode")]
        #[qproperty(bool, lcd_hold, READ, NOTIFY, cxx_name = "lcdHold")]
        #[qproperty(bool, lcd_avls, READ, NOTIFY, cxx_name = "lcdAvls")]
        #[qproperty(
            QString,
            lcd_playback_status,
            READ,
            NOTIFY,
            cxx_name = "lcdPlaybackStatus"
        )]
        #[qproperty(QString, lcd_message, READ, NOTIFY, cxx_name = "lcdMessage")]
        #[qproperty(QString, last_rejection, READ, NOTIFY, cxx_name = "lastRejection")]
        type ApplianceBridge = super::ApplianceBridgeRust;

        #[qinvokable]
        #[cxx_name = "requestOpen"]
        fn request_open(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestClose"]
        fn request_close(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestInsert"]
        fn request_insert(self: Pin<&mut Self>, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "requestRemove"]
        fn request_remove(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestPlay"]
        fn request_play(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestPause"]
        fn request_pause(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestStop"]
        fn request_stop(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestPrevious"]
        fn request_previous(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestNext"]
        fn request_next(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestScanBegin"]
        fn request_scan_begin(self: Pin<&mut Self>, forward: bool) -> bool;

        #[qinvokable]
        #[cxx_name = "requestScanEnd"]
        fn request_scan_end(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestScanStep"]
        fn request_scan_step(self: Pin<&mut Self>, target_ms: i64) -> bool;

        #[qinvokable]
        #[cxx_name = "requestMenuShort"]
        fn request_menu_short(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestMenuLong"]
        fn request_menu_long(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "requestToggleAvls"]
        fn request_toggle_avls(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "setHold"]
        fn set_hold(self: Pin<&mut Self>, enabled: bool) -> bool;

        #[qinvokable]
        #[cxx_name = "setVolume"]
        fn set_volume(self: Pin<&mut Self>, value: f32) -> bool;

        #[qinvokable]
        #[cxx_name = "lidOpened"]
        fn lid_opened(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "lidClosed"]
        fn lid_closed(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "discInserted"]
        fn disc_inserted(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "discRemoved"]
        fn disc_removed(self: Pin<&mut Self>) -> bool;
    }
}

pub struct ApplianceBridgeRust {
    runtime: ApplianceRuntime,
    lid_state: QString,
    disc_state: QString,
    transport_state: QString,
    play_mode: QString,
    hold_enabled: bool,
    avls_enabled: bool,
    volume: f32,
    application_gain: f32,
    machine_error: QString,
    lcd_track_number: i32,
    lcd_elapsed_ms: i64,
    lcd_total_tracks: i32,
    lcd_total_ms: i64,
    lcd_play_mode: QString,
    lcd_hold: bool,
    lcd_avls: bool,
    lcd_playback_status: QString,
    lcd_message: QString,
    last_rejection: QString,
}

impl Default for ApplianceBridgeRust {
    fn default() -> Self {
        let runtime = ApplianceRuntime::new();
        let snapshot = runtime.snapshot();
        Self {
            runtime,
            lid_state: QString::from(snapshot.lid_state.as_str()),
            disc_state: QString::from(snapshot.disc_state.as_str()),
            transport_state: QString::from(snapshot.transport_state.as_str()),
            play_mode: QString::from(snapshot.play_mode.as_str()),
            hold_enabled: snapshot.hold_enabled,
            avls_enabled: snapshot.avls_enabled,
            volume: snapshot.volume,
            application_gain: snapshot.application_gain,
            machine_error: QString::from(snapshot.machine_error.as_str()),
            lcd_track_number: snapshot.lcd_track_number,
            lcd_elapsed_ms: snapshot.lcd_elapsed_ms,
            lcd_total_tracks: snapshot.lcd_total_tracks,
            lcd_total_ms: snapshot.lcd_total_ms,
            lcd_play_mode: QString::from(snapshot.lcd_play_mode.as_str()),
            lcd_hold: snapshot.lcd_hold,
            lcd_avls: snapshot.lcd_avls,
            lcd_playback_status: QString::from(snapshot.lcd_playback_status.as_str()),
            lcd_message: QString::from(snapshot.lcd_message.as_str()),
            last_rejection: QString::default(),
        }
    }
}

macro_rules! update_projection {
    ($object:ident, $field:ident, $value:expr, $signal:ident) => {{
        let value = $value;
        if $object.as_ref().rust().$field != value {
            $object.as_mut().rust_mut().$field = value;
            $object.as_mut().$signal();
        }
    }};
}

impl ffi::ApplianceBridge {
    pub fn request_open(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Open)
    }

    pub fn request_close(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Close)
    }

    pub fn request_insert(self: Pin<&mut Self>, path: &QString) -> bool {
        self.dispatch(Command::Insert(path.to_string().into()))
    }

    pub fn request_remove(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Remove)
    }

    pub fn request_play(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Play)
    }

    pub fn request_pause(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Pause)
    }

    pub fn request_stop(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Stop)
    }

    pub fn request_previous(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Previous)
    }

    pub fn request_next(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::Next)
    }

    pub fn request_scan_begin(self: Pin<&mut Self>, forward: bool) -> bool {
        let direction = if forward {
            ScanDirection::Forward
        } else {
            ScanDirection::Backward
        };
        self.dispatch(Command::ScanBegin(direction))
    }

    pub fn request_scan_end(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::ScanEnd)
    }

    pub fn request_scan_step(self: Pin<&mut Self>, target_ms: i64) -> bool {
        self.dispatch(Command::ScanStep(target_ms))
    }

    pub fn request_menu_short(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::MenuShort)
    }

    pub fn request_menu_long(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::MenuLong)
    }

    pub fn request_toggle_avls(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::ToggleAvls)
    }

    pub fn set_hold(self: Pin<&mut Self>, enabled: bool) -> bool {
        self.dispatch(Command::SetHold(enabled))
    }

    pub fn set_volume(self: Pin<&mut Self>, value: f32) -> bool {
        self.dispatch(Command::SetVolume(value))
    }

    pub fn lid_opened(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::LidOpened)
    }

    pub fn lid_closed(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::LidClosed)
    }

    pub fn disc_inserted(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::DiscInserted)
    }

    pub fn disc_removed(self: Pin<&mut Self>) -> bool {
        self.dispatch(Command::DiscRemoved)
    }

    fn dispatch(mut self: Pin<&mut Self>, command: Command) -> bool {
        let result = self.as_mut().rust_mut().runtime.execute(command);
        let snapshot = self.as_ref().rust().runtime.snapshot();
        self.as_mut().refresh(snapshot);
        let rejection = QString::from(result.as_ref().err().map(String::as_str).unwrap_or(""));
        update_projection!(self, last_rejection, rejection, last_rejection_changed);
        result.is_ok()
    }

    fn refresh(mut self: Pin<&mut Self>, snapshot: ApplianceSnapshot) {
        update_projection!(
            self,
            lid_state,
            QString::from(snapshot.lid_state.as_str()),
            lid_state_changed
        );
        update_projection!(
            self,
            disc_state,
            QString::from(snapshot.disc_state.as_str()),
            disc_state_changed
        );
        update_projection!(
            self,
            transport_state,
            QString::from(snapshot.transport_state.as_str()),
            transport_state_changed
        );
        update_projection!(
            self,
            play_mode,
            QString::from(snapshot.play_mode.as_str()),
            play_mode_changed
        );
        update_projection!(
            self,
            hold_enabled,
            snapshot.hold_enabled,
            hold_enabled_changed
        );
        update_projection!(
            self,
            avls_enabled,
            snapshot.avls_enabled,
            avls_enabled_changed
        );
        update_projection!(self, volume, snapshot.volume, volume_changed);
        update_projection!(
            self,
            application_gain,
            snapshot.application_gain,
            application_gain_changed
        );
        update_projection!(
            self,
            machine_error,
            QString::from(snapshot.machine_error.as_str()),
            machine_error_changed
        );
        update_projection!(
            self,
            lcd_track_number,
            snapshot.lcd_track_number,
            lcd_track_number_changed
        );
        update_projection!(
            self,
            lcd_elapsed_ms,
            snapshot.lcd_elapsed_ms,
            lcd_elapsed_ms_changed
        );
        update_projection!(
            self,
            lcd_total_tracks,
            snapshot.lcd_total_tracks,
            lcd_total_tracks_changed
        );
        update_projection!(
            self,
            lcd_total_ms,
            snapshot.lcd_total_ms,
            lcd_total_ms_changed
        );
        update_projection!(
            self,
            lcd_play_mode,
            QString::from(snapshot.lcd_play_mode.as_str()),
            lcd_play_mode_changed
        );
        update_projection!(self, lcd_hold, snapshot.lcd_hold, lcd_hold_changed);
        update_projection!(self, lcd_avls, snapshot.lcd_avls, lcd_avls_changed);
        update_projection!(
            self,
            lcd_playback_status,
            QString::from(snapshot.lcd_playback_status.as_str()),
            lcd_playback_status_changed
        );
        update_projection!(
            self,
            lcd_message,
            QString::from(snapshot.lcd_message.as_str()),
            lcd_message_changed
        );
    }
}
