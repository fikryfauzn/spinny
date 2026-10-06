import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "CompleteControls"
    Component { id: appComponent; App.Main {} }
    function setup(play) {
        const app = createTemporaryObject(appComponent, null)
        app.controls.windowActive = true
        verify(app.controls.assetReady)
        verify(app.appliance.requestOpen()); tryCompare(app.appliance, "lidState", "Open", 1200)
        verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/audio-runtime-two-track.vdisc"))))
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
        tryCompare(app.appliance, "discState", "Seated", 1000)
        verify(app.appliance.requestClose()); tryCompare(app.appliance, "lidState", "Closed", 1200)
        if (play) verify(app.appliance.requestPlay())
        return app
    }
    function press(app, name, elapsed) {
        let clock = 0
        app.controls.gesture.nowMs = function() { return clock }
        const button = app.controls.buttonForName("BTN_" + name + "_TEST")
        verify(app.controls.beginPress(button))
        clock = elapsed
        return app.controls.finishPress(button)
    }
    function scan(app, forward) {
        let clock = 0
        app.controls.gesture.nowMs = function() { return clock }
        verify(app.controls.beginPress(app.controls.buttonForName(forward ? "BTN_NEXT_TEST" : "BTN_PREVIOUS_TEST")))
        clock = 600
        verify(app.controls.gesture.consumeLong())
        verify(app.controls.gesture.held, "Scan begin lost capture: " + app.controls.captureKind)
        verify(app.controls.gesture.scanning, "Scan begin lost scan flag")
    }
    function test_menu_cycles_and_long_only_toggles_avls() {
        const app = setup(true)
        for (const mode of ["RepeatAll", "Single", "RepeatSingle", "RepeatShuffle", "Normal"]) {
            verify(press(app, "MENU", 599))
            compare(app.appliance.playMode, mode)
        }
        verify(press(app, "MENU", 600))
        verify(app.appliance.avlsEnabled)
        verify(app.appliance.lcdAvls)
        compare(app.appliance.playMode, "Normal")
    }
    function test_scan_live_position_release_and_hold_cleanup() {
        const app = setup(true)
        audioTestDriver.setPosition(123)
        scan(app, true)
        compare(app.appliance.transportState, "SeekingForward")
        verify(app.controls.gesture.scanTick(), app.appliance.lastRejection + "/" + app.appliance.machineError)
        compare(app.appliance.lcdElapsedMs, 1723)
        verify(app.controls.finishPress(app.controls.buttonForName("BTN_NEXT_TEST")))
        compare(app.appliance.transportState, "Playing")
        compare(app.appliance.lcdTrackNumber, 1)
        scan(app, false)
        verify(app.controls.gesture.scanTick())
        compare(app.appliance.lcdElapsedMs, 123)
        verify(app.appliance.setHold(true))
        compare(app.controls.captureKind, "")
        compare(app.appliance.transportState, "Playing")
        verify(app.appliance.lcdHold)
    }
    function test_eof_and_failure_cancel_scan_data() {
        return [{tag:"normal-eof", event:1, repeat:false, state:"Playing", track:2},
                {tag:"same-track-replay", event:1, repeat:true, state:"Playing", track:1},
                {tag:"failure", event:2, repeat:false, state:"Stopped", track:-1}]
    }
    function test_eof_and_failure_cancel_scan(data) {
        const app = setup(true)
        if (data.repeat) { verify(press(app,"MENU",0)); verify(press(app,"MENU",0)); verify(press(app,"MENU",0)) }
        scan(app, true)
        audioTestDriver.queueEvent(data.event)
        tryCompare(app.controls, "captureKind", "", 500)
        compare(app.appliance.transportState, data.state)
        compare(app.appliance.lcdTrackNumber, data.track)
        verify(!app.controls.finishPress(app.controls.buttonForName("BTN_NEXT_TEST")))
        verify(!app.controls.gesture.scanTimer.running)
    }
    function test_inspection_changes_cancel_capture() {
        const app = setup(true)
        scan(app, true)
        app.inspection.cutawayEnabled = true
        compare(app.controls.captureKind, "")
        compare(app.appliance.transportState, "Playing")
    }
    function test_physical_hold_switch_unlocks_and_rejects_buttons() {
        const app = setup(false)
        tryCompare(app.contentItem, "width", app.width, 500)
        const knob = app.controls.resolve("HOLD_KNOB_TEST")
        function toggle() {
            const point = app.controls.view3d.mapFrom3DScene(knob.scenePosition)
            verify(app.controls.beginRail(knob, point.x, point.y))
            verify(app.controls.finishRail(point.x, point.y))
        }
        toggle()
        verify(app.appliance.holdEnabled)
        verify(app.appliance.lcdHold)
        for (const name of ["PLAY", "OPEN", "MENU"]) {
            verify(!press(app, name, 0))
            compare(app.appliance.lcdMessage, "Hold")
            compare(app.appliance.transportState, "Stopped")
        }
        toggle()
        verify(!app.appliance.holdEnabled)
        verify(!app.appliance.lcdHold)
        compare(app.appliance.lcdTotalTracks, 2)
        verify(press(app, "PLAY", 0))
    }
}
