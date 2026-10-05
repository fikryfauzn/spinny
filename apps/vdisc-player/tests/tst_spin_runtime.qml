import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "SpinRuntime"
    Component { id: appComponent; App.Main {} }
    function setup() {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        verify(app.spin !== undefined)
        verify(app.inspection !== undefined)
        verify(app.spin.assetReady)
        verify(app.inspection.assetReady)
        return app
    }
    function loadedApp() {
        const app = setup()
        verify(app.appliance.requestOpen())
        tryCompare(app.appliance, "lidState", "Open", 1200)
        verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl(
            "../../../tests/fixtures/vdisc/transport-two-track.vdisc"))))
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
        compare(app.appliance.discState, "Inserting")
        compare(app.spin.speed, 0)
        tryCompare(app.appliance, "discState", "Seated", 1000)
        verify(app.appliance.requestClose())
        compare(app.spin.speed, 0)
        tryCompare(app.appliance, "lidState", "Closed", 1200)
        return app
    }
    function test_real_transport_and_production_frames() {
        const app = loadedApp()
        const frame = app.spin.frameCount
        verify(app.appliance.requestPlay())
        tryCompare(app.spin, "speed", 180, 650)
        verify(app.spin.frameCount > frame)
        verify(app.spin.angle > 0)
        fuzzyCompare(app.spin.discModel.eulerRotation.y, app.spin.angle, 0.000001)
        verify(!app.appliance.requestOpen())
        compare(app.appliance.lidState, "Closed")
        compare(app.spin.speed, 180)
        verify(app.appliance.requestPause())
        wait(120)
        verify(app.spin.speed > 0 && app.spin.speed < 180)
        verify(!app.appliance.requestPlay())
        verify(!app.appliance.requestOpen())
        tryCompare(app.spin, "speed", 0, 450)
        const angle = app.spin.angle
        wait(80)
        compare(app.spin.angle, angle)
        verify(app.appliance.requestPause())
        tryCompare(app.spin, "speed", 180, 650)
        verify(app.appliance.requestStop())
        wait(100)
        verify(app.spin.speed > 0 && app.spin.speed < 180)
        tryCompare(app.spin, "speed", 0, 450)
        verify(!app.spin.frameRunning)
    }
    function test_stop_then_open_is_immediate_and_completion_is_natural() {
        const app = loadedApp()
        verify(app.appliance.requestPlay())
        tryCompare(app.spin, "speed", 180, 650)
        verify(app.appliance.requestStop())
        const angle = app.spin.angle
        verify(app.appliance.requestOpen())
        compare(app.appliance.lidState, "Opening")
        compare(app.spin.speed, 0)
        verify(!app.spin.rampRunning)
        compare(app.spin.angle, angle)
        wait(100)
        compare(app.appliance.lidState, "Opening")
        compare(app.spin.angle, angle)
        tryCompare(app.appliance, "lidState", "Open", 1000)
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(0, 0.014, 0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.18, 0.014, 0)))
        compare(app.appliance.discState, "Removing")
        compare(app.spin.speed, 0)
        tryCompare(app.appliance, "discState", "Absent", 1000)
        compare(app.spin.angle, 0)
        compare(app.spin.discModel.eulerRotation.y, 0)
    }
    function test_cutaway_preserves_interlocks_and_controls() {
        const app = loadedApp()
        const lid = app.inspection.lidModel
        const hingeAngle = app.lidPivot.eulerRotation.x
        verify(app.inspection.toggleCutaway())
        verify(!lid.visible)
        compare(app.appliance.lidState, "Closed")
        compare(app.lidPivot.eulerRotation.x, hingeAngle)
        verify(!app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(0, 0.014, 0)))
        const button = app.controls.buttonForName("BTN_PLAY_TEST")
        verify(app.controls.beginPress(button))
        verify(app.controls.finishPress(button))
        tryCompare(app.spin, "speed", 180, 650)
        verify(app.inspection.toggleCutaway())
        verify(lid.visible)
        compare(app.appliance.transportState, "Playing")
        verify(app.appliance.setHold(true))
        verify(!app.appliance.requestStop())
        compare(app.spin.speed, 180)
        compare(app.appliance.transportState, "Playing")
    }
    function test_empty_staged_and_shortcut_scoping() {
        const app = setup()
        verify(!app.appliance.requestPlay())
        compare(app.spin.speed, 0)
        verify(app.appliance.requestOpen())
        tryCompare(app.appliance, "lidState", "Open", 1200)
        verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl(
            "../../../tests/fixtures/vdisc/transport-two-track.vdisc"))))
        verify(!app.appliance.requestPlay())
        compare(app.spin.speed, 0)
        verify(!app.spin.frameRunning)
        app.inspection.windowActive = true
        verify(app.inspection.shortcut.enabled)
        app.inspection.filePickerVisible = true
        verify(!app.inspection.shortcut.enabled)
        app.inspection.filePickerVisible = false
        app.inspection.windowActive = false
        verify(!app.inspection.shortcut.enabled)
        const button = app.controls.buttonForName("BTN_OPEN_TEST")
        app.controls.windowActive = true
        verify(app.controls.beginPress(button))
        app.discDeck.filePickerOpened()
        compare(app.controls.pressedName, "")
    }
}
