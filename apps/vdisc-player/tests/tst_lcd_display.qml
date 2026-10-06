import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "LcdDisplay"
    when: windowShown
    Component { id: displayComponent; App.LcdDisplay {} }
    QtObject {
        id: a
        property int lcdTrackNumber: 2
        property double lcdElapsedMs: 12345
        property int lcdTotalTracks: 6
        property double lcdTotalMs: 321000
        property string lcdPlaybackStatus: "Playing"
        property string lcdPlayMode: "Normal"
        property bool lcdHold: false
        property bool lcdAvls: false
        property string lcdMessage: ""
        property string lastRejection: "/private/path HoldEnabled decode diagnostic"
    }
    property var display
    function init() {
        a.lcdTrackNumber = 2; a.lcdElapsedMs = 12345
        a.lcdTotalTracks = 6; a.lcdTotalMs = 321000
        a.lcdPlaybackStatus = "Playing"; a.lcdPlayMode = "Normal"
        a.lcdHold = false; a.lcdAvls = false; a.lcdMessage = ""
        display = createTemporaryObject(displayComponent, this, {appliance: a})
        verify(display !== null)
    }
    function test_format_boundaries() {
        for (const pair of [[0,"00:00"],[12345,"00:12"],[3599999,"59:59"],[3600000,"60:00"],[7200000000,"120000:00"],[-1,"--:--"],[NaN,"--:--"],[Infinity,"--:--"],["12000","--:--"],[null,"--:--"]])
            compare(display.formatTime(pair[0]), pair[1])
        for (const pair of [[1,"01"],[99,"99"],[100,"100"],[0,"--"],[-1,"--"],[1.5,"--"],[Infinity,"--"],["2","--"],[null,"--"]])
            compare(display.formatCount(pair[0]), pair[1])
    }
    function test_transport_data() {
        return [{tag:"stop",state:"Stopped",label:"STOP",primary:"06  05:21"},
            {tag:"play",state:"Playing",label:"PLAY",primary:"02  00:12"},
            {tag:"pause",state:"Paused",label:"PAUSE",primary:"02  00:12"},
            {tag:"forward",state:"SeekingForward",label:"SCAN+",primary:"02  00:12"},
            {tag:"backward",state:"SeekingBackward",label:"SCAN−",primary:"02  00:12"},
            {tag:"unknown",state:"Bogus",label:"--",primary:"--  --:--"}]
    }
    function test_transport(data) {
        a.lcdPlaybackStatus = data.state
        compare(display.primaryText, data.primary); compare(display.statusText, data.label)
    }
    function test_modes() {
        for (const pair of [["Normal",""],["RepeatAll","REP"],["Single","1"],["RepeatSingle","REP 1"],["RepeatShuffle","SHUF REP"],["Bogus","--"]]) {
            a.lcdPlayMode = pair[0]; compare(display.modeText,pair[1])
        }
    }
    function test_messages_keep_indicators_not_raw_diagnostics() {
        a.lcdHold = true; a.lcdAvls = true; a.lcdPlayMode = "RepeatShuffle"
        for (const pair of [["Hold","HOLD"],["InvalidDisc","INVALID DISC"],["UnreadableDisc","DISC ERROR"],["PlaybackFailure","PLAY ERROR"],["AudioOutputFailure","AUDIO ERROR"],["Bogus","ERROR"]]) {
            a.lcdMessage = pair[0]
            compare(display.primaryText, pair[1]); compare(display.holdText,"HOLD")
            compare(display.avlsText,"AVLS"); compare(display.modeText,"SHUF REP")
            compare(display.statusText,"PLAY")
            verify(display.primaryText.indexOf("/private/") < 0)
        }
        a.lcdMessage = ""; compare(display.primaryText,"02  00:12")
    }
    function test_empty_and_long_fields_remain_inside_screen() {
        a.lcdTrackNumber = -1; a.lcdElapsedMs = -1
        compare(display.primaryText,"--  --:--")
        a.lcdTrackNumber = 255; a.lcdElapsedMs = 9223372036854775807
        a.lcdPlayMode = "RepeatShuffle"; a.lcdHold = true; a.lcdAvls = true
        compare(display.width,900); compare(display.height,120); verify(display.clip)
        for (const child of display.children) {
            verify(child.x >= 0 && child.y >= 0)
            verify(child.x + child.width <= 900 && child.y + child.height <= 120)
        }
        verify(display.primaryText.indexOf("NaN") < 0)
    }
}
