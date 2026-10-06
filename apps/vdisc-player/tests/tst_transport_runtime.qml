import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "TransportRuntime"
    Component { id: appComponent; App.Main {} }

    function setup() {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        verify(app.controls !== undefined)
        verify(app.controls.assetReady)
        return app
    }
    function click(app, name) {
        const button = app.controls.buttonForName("BTN_" + name + "_TEST")
        verify(app.controls.beginPress(button))
        return app.controls.finishPress(button)
    }
    function loadedApp() {
        const app = setup()
        verify(click(app, "OPEN"))
        tryCompare(app.appliance, "lidState", "Open", 1200)
        const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/transport-two-track.vdisc"))
        verify(app.discDeck.selectDiscUrl(url))
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)), app.appliance.lastRejection)
        tryCompare(app.appliance, "discState", "Seated", 1000)
        compare(app.appliance.lcdTotalTracks, 2)
        verify(click(app, "OPEN"))
        tryCompare(app.appliance, "lidState", "Closed", 1200)
        return app
    }
    function test_transport_and_track_steps_through_real_bridge() {
        const app = loadedApp()
        verify(click(app, "PLAY"))
        compare(app.appliance.transportState, "Playing")
        verify(click(app, "PAUSE"))
        compare(app.appliance.transportState, "Paused")
        verify(!click(app, "PLAY"))
        compare(app.appliance.transportState, "Paused")
        verify(click(app, "PAUSE"))
        compare(app.appliance.transportState, "Playing")
        compare(app.appliance.lcdTrackNumber, 1)
        verify(click(app, "NEXT"))
        compare(app.appliance.lcdTrackNumber, 2)
        verify(click(app, "PREVIOUS"))
        compare(app.appliance.lcdTrackNumber, 1)
        verify(!click(app, "OPEN"))
        compare(app.appliance.lidState, "Closed")
        verify(!app.openingAnimation.running)
        verify(app.appliance.lastRejection.length > 0)
        verify(click(app, "STOP"))
        compare(app.appliance.transportState, "Stopped")
        verify(click(app, "OPEN"))
        tryCompare(app.appliance, "lidState", "Open", 1200)
    }
    function test_hold_blocks_all_six_controls_data() {
        return ["PLAY", "PAUSE", "STOP", "PREVIOUS", "NEXT", "OPEN"].map(
            function(name) { return {tag: name, name: name} })
    }
    function test_hold_blocks_all_six_controls(data) {
        const app = loadedApp()
        if (data.name === "PAUSE" || data.name === "STOP")
            verify(click(app, "PLAY"))
        verify(app.appliance.setHold(true))
        const transport = app.appliance.transportState
        const track = app.appliance.lcdTrackNumber
        const button = app.controls.buttonForName("BTN_" + data.name + "_TEST")
        const restY = button.y
        const restZ = button.z
        verify(app.controls.beginPress(button))
        wait(80)
        if (data.name === "PREVIOUS" || data.name === "NEXT")
            fuzzyCompare(button.z, restZ - 0.0006, 0.000001)
        else
            fuzzyCompare(button.y, restY - 0.0006, 0.000001)
        verify(!app.controls.finishPress(button))
        compare(app.appliance.transportState, transport)
        compare(app.appliance.lcdMessage, "Hold")
        compare(app.appliance.lcdTrackNumber, -1)
        compare(app.appliance.lidState, "Closed")
        verify(app.appliance.lastRejection.length > 0)
        wait(120)
        fuzzyCompare(button.y, restY, 0.000001)
        fuzzyCompare(button.z, restZ, 0.000001)
        verify(app.appliance.setHold(false))
        compare(app.appliance.lcdTrackNumber, track)
        compare(app.appliance.transportState, transport)
    }
    function test_empty_transport_rejected_data() {
        return ["PLAY", "PAUSE", "STOP", "PREVIOUS", "NEXT"].map(
            function(name) { return {tag: name, name: name} })
    }
    function test_empty_transport_rejected(data) {
        const app = setup()
        verify(!click(app, data.name))
        compare(app.appliance.discState, "Absent")
        compare(app.appliance.transportState, "Stopped")
        verify(app.appliance.lastRejection.length > 0)
    }
    function test_open_during_transition_does_not_reverse_motion() {
        const app = setup()
        verify(click(app, "OPEN"))
        wait(100)
        const angle = app.lidPivot.eulerRotation.x
        verify(!click(app, "OPEN"))
        compare(app.appliance.lidState, "Opening")
        wait(80)
        verify(app.lidPivot.eulerRotation.x < angle)
        tryCompare(app.appliance, "lidState", "Open", 1000)
        verify(click(app, "OPEN"))
        wait(100)
        verify(!click(app, "OPEN"))
        compare(app.appliance.lidState, "Closing")
        tryCompare(app.appliance, "lidState", "Closed", 1000)
    }
    function test_disc_and_control_gestures_have_separate_hits() {
        const app = setup()
        verify(click(app, "OPEN"))
        tryCompare(app.appliance, "lidState", "Open", 1200)
        const button = app.controls.buttonForName("BTN_PLAY_TEST")
        verify(!app.discDeck.tryBeginDrag(button, Qt.vector3d(0, 0.014, 0)))
        verify(!app.controls.beginPress(app.discDeck.discModel))
        verify(!app.controls.beginPress(app.findUniqueNamedNode(app.greybox, "BODY_TEST")))
        verify(app.controls.beginPress(button))
        app.discDeck.filePickerOpened()
        compare(app.controls.pressedName, "")
        verify(!app.controls.finishPress(button))
        compare(app.appliance.transportState, "Stopped")
    }
    function test_window_inactive_cancels_press() {
        const app = setup()
        app.controls.windowActive = true
        const button = app.controls.buttonForName("BTN_OPEN_TEST")
        verify(app.controls.beginPress(button))
        app.controls.windowActive = false
        compare(app.controls.pressedName, "")
        verify(!app.controls.finishPress(button))
        compare(app.appliance.lidState, "Closed")
    }
}
