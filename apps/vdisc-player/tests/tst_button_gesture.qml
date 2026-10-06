import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "ButtonGesture"
    property real clock: 0
    Component { id: helper; App.ButtonGesture {} }
    Component {
        id: recorder
        QtObject {
            property string transportState: "Playing"
            property string lidState: "Closed"
            property bool holdEnabled: false
            property bool accepted: true
            property var calls: []
            function record(action) { calls = calls.concat([action]); return accepted }
            function requestMenuShort() { return record("MODE") }
            function requestMenuLong() { return record("AVLS") }
            function requestPrevious() { return record("PREVIOUS") }
            function requestNext() { return record("NEXT") }
            function requestScanBegin(forward) {
                if (!record(forward ? "FORWARD" : "BACKWARD")) return false
                transportState = forward ? "SeekingForward" : "SeekingBackward"
                return true
            }
            function requestScanRelative(delta) { return record(delta) }
            function requestScanEnd() { record("END"); transportState = "Playing"; return true }
        }
    }
    function setup() {
        clock = 0
        const bridge = createTemporaryObject(recorder, null)
        const gesture = createTemporaryObject(helper, null,
            {appliance: bridge, nowMs: function() { return clock }})
        return {bridge: bridge, gesture: gesture}
    }
    function test_threshold_data() {
        return [{tag:"menu-short", name:"BTN_MENU_TEST", time:599, calls:"MODE"},
                {tag:"menu-long", name:"BTN_MENU_TEST", time:600, calls:"AVLS"},
                {tag:"next-short", name:"BTN_NEXT_TEST", time:599, calls:"NEXT"},
                {tag:"next-long", name:"BTN_NEXT_TEST", time:600, calls:"FORWARD,END"},
                {tag:"previous-short", name:"BTN_PREVIOUS_TEST", time:599, calls:"PREVIOUS"},
                {tag:"previous-long", name:"BTN_PREVIOUS_TEST", time:600, calls:"BACKWARD,END"}]
    }
    function test_threshold(data) {
        const s = setup()
        verify(s.gesture.begin(data.name))
        clock = data.time
        verify(s.gesture.finish(true))
        compare(s.bridge.calls.join(","), data.calls)
        verify(!s.gesture.finish(true))
    }
    function test_long_consumed_once_and_relative_direction() {
        const s = setup()
        verify(s.gesture.begin("BTN_PREVIOUS_TEST"))
        verify(!s.gesture.begin("BTN_NEXT_TEST"))
        clock = 600
        verify(s.gesture.consumeLong())
        verify(!s.gesture.consumeLong())
        verify(s.gesture.scanTick())
        compare(s.bridge.calls.join(","), "BACKWARD,-1600")
        verify(s.gesture.finish(true))
        compare(s.bridge.calls.join(","), "BACKWARD,-1600,END")
        verify(!s.gesture.scanTimer.running)
    }
    function test_rejected_long_never_falls_back_to_short() {
        const s = setup()
        s.bridge.accepted = false
        verify(s.gesture.begin("BTN_MENU_TEST"))
        clock = 600
        verify(!s.gesture.consumeLong())
        verify(!s.gesture.finish(true))
        compare(s.bridge.calls.join(","), "AVLS")
    }
    function test_cancel_and_failed_tick_stop_all_timers() {
        const s = setup()
        verify(!s.gesture.begin("INVALID"))
        verify(s.gesture.begin("BTN_MENU_TEST"))
        s.gesture.cancel()
        clock = 1000
        verify(!s.gesture.consumeLong())
        verify(!s.gesture.finish(true))
        compare(s.bridge.calls.length, 0)
        verify(s.gesture.begin("BTN_NEXT_TEST"))
        clock = 1600
        verify(s.gesture.consumeLong())
        s.bridge.accepted = false
        verify(!s.gesture.scanTick())
        verify(!s.gesture.held)
        verify(!s.gesture.scanTimer.running)
        compare(s.bridge.calls.join(","), "FORWARD,1600,END")
    }
    function test_hold_and_eof_cleanup_data() {
        return [{tag:"hold", hold:true, transport:"SeekingForward", calls:"FORWARD,END"},
                {tag:"replay-eof", hold:false, transport:"Playing", calls:"FORWARD"},
                {tag:"failure", hold:false, transport:"Stopped", calls:"FORWARD"}]
    }
    function test_hold_and_eof_cleanup(data) {
        const s = setup()
        verify(s.gesture.begin("BTN_NEXT_TEST"))
        clock = 600
        verify(s.gesture.consumeLong())
        verify(s.gesture.scanning)
        s.bridge.holdEnabled = data.hold
        s.bridge.transportState = data.transport
        tryCompare(s.gesture, "held", false)
        verify(!s.gesture.finish(true))
        verify(!s.gesture.scanTimer.running)
        compare(s.bridge.calls.join(","), data.calls)
    }
    function test_real_timer_smoke() {
        const s = setup()
        s.gesture.nowMs = function() { return Date.now() }
        verify(s.gesture.begin("BTN_MENU_TEST"))
        tryCompare(s.gesture, "longConsumed", true, 1500)
        compare(s.bridge.calls.join(","), "AVLS")
        s.gesture.finish(true)
        compare(s.bridge.calls.join(","), "AVLS")
    }
}
