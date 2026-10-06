import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "AudioControls"
    when: windowShown
    Component { id: appComponent; App.Main {} }
    function setup(play) {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        tryCompare(app.contentItem, "width", app.width, 500)
        app.controls.windowActive = true
        if (play) {
            verify(app.appliance.requestOpen()); tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/transport-two-track.vdisc"))))
            verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
            tryCompare(app.appliance, "discState", "Seated", 1000)
            verify(app.appliance.requestClose()); tryCompare(app.appliance, "lidState", "Closed", 1200)
            verify(app.appliance.requestPlay())
        }
        return app
    }
    function volume(app, x) {
        const rail = app.controls.resolve("VOLUME_RAIL_TEST")
        const scene = rail.parent.mapPositionToScene(Qt.vector3d(x, 0.013, 0.0735))
        const point = app.controls.view3d.mapFrom3DScene(scene)
        verify(app.controls.beginRail(rail, point.x, point.y))
        verify(app.controls.finishRail(point.x, point.y))
    }
    function menu(app) {
        let clock = 0
        app.controls.gesture.nowMs = function() { return clock }
        const button = app.controls.buttonForName("BTN_MENU_TEST")
        verify(app.controls.beginPress(button))
        clock = 600
        return app.controls.finishPress(button)
    }
    function test_physical_volume_changes_live_session_gain() {
        const app = setup(true)
        volume(app, 0.0375)
        fuzzyCompare(app.appliance.volume, 0.25, 0.00001)
        fuzzyCompare(audioTestDriver.gain(), 0.25, 0.00001)
        volume(app, 0.01)
        compare(app.appliance.volume, 0)
        compare(audioTestDriver.gain(), 0)
        compare(app.appliance.transportState, "Playing")
    }
    function test_repeated_avls_clamp_restores_knob_without_notify() {
        const app = setup(true)
        volume(app, 0.06)
        verify(menu(app))
        verify(app.appliance.avlsEnabled)
        fuzzyCompare(audioTestDriver.gain(), 0.75, 0.00001)
        for (let i = 0; i < 3; ++i) {
            volume(app, 0.06)
            fuzzyCompare(app.controls.resolve("VOLUME_KNOB_TEST").x, 0.0485, 0.000001)
            fuzzyCompare(app.appliance.volume, 0.75, 0.00001)
        }
        verify(menu(app))
        verify(!app.appliance.avlsEnabled)
        fuzzyCompare(app.appliance.volume, 0.75, 0.00001)
    }
    function test_hold_allows_volume_rejects_menu() {
        const app = setup(false)
        verify(app.appliance.setHold(true))
        volume(app, 0.06)
        compare(app.appliance.volume, 1)
        verify(!menu(app))
        verify(!app.appliance.avlsEnabled)
        verify(app.appliance.lastRejection.length > 0)
    }
    function test_real_file_dialog_blocks_physical_input() {
        const app = setup(false)
        const button = app.controls.buttonForName("BTN_OPEN_TEST")
        verify(app.controls.beginPress(button))
        mouseClick(app.discDeck, 70, 35)
        tryCompare(app.discDeck, "filePickerVisible", true, 500)
        verify(!app.controls.inputActive)
        compare(app.controls.captureKind, "")
        verify(!app.controls.beginPress(button))
        let dialog = null
        for (const object of app.discDeck.data) {
            if (object.title === "Choose a VDISC") dialog = object
        }
        verify(dialog !== null)
        dialog.close()
        tryCompare(app.discDeck, "filePickerVisible", false, 500)
        verify(app.controls.inputActive)
        mouseClick(app.inspection, app.width - 90, 35)
        verify(app.inspection.cutawayEnabled)
    }
    function test_machine_error_wins_over_stale_rejection() {
        const app = setup(true)
        verify(!app.appliance.requestOpen())
        audioTestDriver.queueEvent(2)
        tryCompare(app.appliance, "machineError", "AudioOutputFailure", 500)
        compare(app.discDeck.statusText, "AudioOutputFailure")
        verify(app.appliance.lastRejection.length > 0)
    }
}
