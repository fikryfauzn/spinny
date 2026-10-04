use vdisc_appliance::{
    ApplianceErrorState, DiscState, LcdMessage, LcdPlaybackStatus, LidState, PlayMode,
    PlaybackPosition, TransportState,
};

use crate::ApplianceRuntime;

#[derive(Debug, Clone, PartialEq)]
pub struct ApplianceSnapshot {
    pub lid_state: String,
    pub disc_state: String,
    pub transport_state: String,
    pub play_mode: String,
    pub hold_enabled: bool,
    pub avls_enabled: bool,
    pub volume: f32,
    pub application_gain: f32,
    pub machine_error: String,
    pub lcd_track_number: i32,
    pub lcd_elapsed_ms: i64,
    pub lcd_total_tracks: i32,
    pub lcd_total_ms: i64,
    pub lcd_play_mode: String,
    pub lcd_hold: bool,
    pub lcd_avls: bool,
    pub lcd_playback_status: String,
    pub lcd_message: String,
}

impl ApplianceRuntime {
    pub fn snapshot(&self) -> ApplianceSnapshot {
        let controller = &self.controller;
        let lcd = controller.lcd_snapshot(self.backend.lcd_facts(), None);
        ApplianceSnapshot {
            lid_state: lid_state(controller.lid_state()).into(),
            disc_state: disc_state(controller.disc_state()).into(),
            transport_state: transport_state(controller.transport_state()).into(),
            play_mode: play_mode(controller.play_mode()).into(),
            hold_enabled: controller.hold_enabled(),
            avls_enabled: controller.avls_enabled(),
            volume: controller.volume().normalized(),
            application_gain: self.backend.application_gain(controller),
            machine_error: controller
                .error_state()
                .map(machine_error)
                .unwrap_or("")
                .into(),
            lcd_track_number: lcd.track_number().map(i32::from).unwrap_or(-1),
            lcd_elapsed_ms: optional_millis(lcd.elapsed_time()),
            lcd_total_tracks: lcd.total_tracks().map(i32::from).unwrap_or(-1),
            lcd_total_ms: optional_millis(lcd.total_time()),
            lcd_play_mode: play_mode(lcd.play_mode()).into(),
            lcd_hold: lcd.hold_indicator(),
            lcd_avls: lcd.avls_indicator(),
            lcd_playback_status: lcd_status(lcd.playback_status()).into(),
            lcd_message: lcd.message().map(lcd_message).unwrap_or("").into(),
        }
    }
}

fn optional_millis(position: Option<PlaybackPosition>) -> i64 {
    position
        .map(|position| i64::try_from(position.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(-1)
}

fn lid_state(value: LidState) -> &'static str {
    match value {
        LidState::Closed => "Closed",
        LidState::Opening => "Opening",
        LidState::Open => "Open",
        LidState::Closing => "Closing",
    }
}

fn disc_state(value: DiscState) -> &'static str {
    match value {
        DiscState::Absent => "Absent",
        DiscState::Inserting => "Inserting",
        DiscState::Seated => "Seated",
        DiscState::Removing => "Removing",
    }
}

fn transport_state(value: TransportState) -> &'static str {
    match value {
        TransportState::Stopped => "Stopped",
        TransportState::Playing => "Playing",
        TransportState::Paused => "Paused",
        TransportState::SeekingForward => "SeekingForward",
        TransportState::SeekingBackward => "SeekingBackward",
    }
}

fn play_mode(value: PlayMode) -> &'static str {
    match value {
        PlayMode::Normal => "Normal",
        PlayMode::RepeatAll => "RepeatAll",
        PlayMode::Single => "Single",
        PlayMode::RepeatSingle => "RepeatSingle",
        PlayMode::RepeatShuffle => "RepeatShuffle",
    }
}

fn machine_error(value: ApplianceErrorState) -> &'static str {
    match value {
        ApplianceErrorState::InvalidDisc => "InvalidDisc",
        ApplianceErrorState::UnreadableDisc => "UnreadableDisc",
        ApplianceErrorState::PlaybackFailure => "PlaybackFailure",
        ApplianceErrorState::AudioOutputFailure => "AudioOutputFailure",
        _ => "UnknownMachineError",
    }
}

fn lcd_status(value: LcdPlaybackStatus) -> &'static str {
    match value {
        LcdPlaybackStatus::Stopped => "Stopped",
        LcdPlaybackStatus::Playing => "Playing",
        LcdPlaybackStatus::Paused => "Paused",
        LcdPlaybackStatus::SeekingForward => "SeekingForward",
        LcdPlaybackStatus::SeekingBackward => "SeekingBackward",
    }
}

fn lcd_message(value: LcdMessage) -> &'static str {
    match value {
        LcdMessage::Hold => "Hold",
        LcdMessage::InvalidDisc => "InvalidDisc",
        LcdMessage::UnreadableDisc => "UnreadableDisc",
        LcdMessage::PlaybackFailure => "PlaybackFailure",
        LcdMessage::AudioOutputFailure => "AudioOutputFailure",
    }
}

#[cfg(test)]
mod tests {
    use vdisc_appliance::PlaybackPosition;

    #[test]
    fn huge_position_clamps_to_qml_integer_range() {
        assert_eq!(
            super::optional_millis(Some(PlaybackPosition::from_millis(u64::MAX))),
            i64::MAX
        );
    }
}
