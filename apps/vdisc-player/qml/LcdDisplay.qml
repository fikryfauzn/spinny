import QtQuick

Rectangle {
    id: display
    required property var appliance
    width: 900
    height: 120
    color: "#a7b49b"
    clip: true
    readonly property string statusText: statusLabel(appliance.lcdPlaybackStatus)
    readonly property string modeText: modeLabel(appliance.lcdPlayMode)
    readonly property string holdText: appliance.lcdHold ? "HOLD" : ""
    readonly property string avlsText: appliance.lcdAvls ? "AVLS" : ""
    readonly property string primaryText: {
        if (appliance.lcdMessage !== "")
            return messageLabel(appliance.lcdMessage)
        if (statusText === "--")
            return "--  --:--"
        const stopped = appliance.lcdPlaybackStatus === "Stopped"
        return formatCount(stopped ? appliance.lcdTotalTracks : appliance.lcdTrackNumber)
            + "  " + formatTime(stopped ? appliance.lcdTotalMs : appliance.lcdElapsedMs)
    }

    function formatCount(value) {
        return typeof value === "number" && isFinite(value) && value > 0
            && Math.floor(value) === value ? String(value).padStart(2, "0") : "--"
    }
    function formatTime(value) {
        if (typeof value !== "number" || !isFinite(value) || value < 0)
            return "--:--"
        const seconds = Math.floor(value / 1000)
        return String(Math.floor(seconds / 60)).padStart(2, "0")
            + ":" + String(seconds % 60).padStart(2, "0")
    }
    function statusLabel(value) {
        switch (value) {
        case "Stopped": return "STOP"
        case "Playing": return "PLAY"
        case "Paused": return "PAUSE"
        case "SeekingForward": return "SCAN+"
        case "SeekingBackward": return "SCAN−"
        default: return "--"
        }
    }
    function modeLabel(value) {
        switch (value) {
        case "Normal": return ""
        case "RepeatAll": return "REP"
        case "Single": return "1"
        case "RepeatSingle": return "REP 1"
        case "RepeatShuffle": return "SHUF REP"
        default: return "--"
        }
    }
    function messageLabel(value) {
        switch (value) {
        case "Hold": return "HOLD"
        case "InvalidDisc": return "INVALID DISC"
        case "UnreadableDisc": return "DISC ERROR"
        case "PlaybackFailure": return "PLAY ERROR"
        case "AudioOutputFailure": return "AUDIO ERROR"
        default: return "ERROR"
        }
    }
    Text {
        x: 16; y: 0; width: 868; height: 72
        text: display.primaryText
        color: "#253126"
        font.family: "monospace"
        font.pixelSize: 48
        verticalAlignment: Text.AlignVCenter
        clip: true
    }
    Text {
        x: 16; y: 72; width: 868; height: 48
        text: [display.statusText, display.modeText, display.holdText, display.avlsText]
            .filter(function(value) { return value !== "" }).join("   ")
        color: "#253126"
        font.family: "monospace"
        font.pixelSize: 22
        verticalAlignment: Text.AlignVCenter
        clip: true
    }
}
