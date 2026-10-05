import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "AudioRuntime"
    when: windowShown
    Component { id: appComponent; App.Main {} }
    function playing() {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        const a = app.appliance
        verify(a.requestOpen()); tryCompare(a, "lidState", "Open", 1000)
        verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/transport-two-track.vdisc"))))
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
        tryCompare(a, "discState", "Seated", 1000)
        verify(a.requestClose()); tryCompare(a, "lidState", "Closed", 1000)
        verify(a.requestPlay())
        return app
    }
    function test_timer_advances_eof_and_stops_without_click() {
        const app = playing()
        verify(app.audioRuntime.pollingTimer.running)
        audioTestDriver.queueEvent(1)
        tryCompare(app.appliance, "lcdTrackNumber", 2, 500)
        audioTestDriver.queueEvent(1)
        tryCompare(app.appliance, "transportState", "Stopped", 500)
        compare(app.appliance.lcdTrackNumber, 1)
        tryCompare(app.spin, "speed", 0, 600)
    }
    function test_paused_position_and_idle_rejection_survive_timer() {
        const app = playing()
        audioTestDriver.setPosition(7)
        tryCompare(app.appliance, "lcdElapsedMs", 7, 500)
        verify(app.appliance.requestPause())
        verify(!app.appliance.requestOpen())
        const rejected = app.appliance.lastRejection
        wait(100)
        compare(app.appliance.lcdElapsedMs, 7)
        compare(app.appliance.lastRejection, rejected)
        audioTestDriver.queueEvent(1)
        wait(100)
        compare(app.appliance.lcdTrackNumber, 1)
        compare(app.appliance.transportState, "Paused")
        verify(app.appliance.requestPause())
        tryCompare(app.appliance, "lcdTrackNumber", 2, 500)
    }
    function test_failure_data() { return [{tag: "device", event: 2, error: "AudioOutputFailure"}, {tag: "decode", event: 3, error: "PlaybackFailure"}] }
    function test_failure(data) {
        const app = playing()
        audioTestDriver.queueEvent(data.event)
        tryCompare(app.appliance, "machineError", data.error, 500)
        compare(app.appliance.transportState, "Stopped")
        tryCompare(app.spin, "speed", 0, 600)
        compare(app.appliance.discState, "Seated")
        verify(app.appliance.setVolume(0.2))
        compare(app.appliance.machineError, data.error)
    }
    function test_hidden_window_keeps_application_pump() {
        const app = playing()
        app.visible = false
        audioTestDriver.queueEvent(2)
        tryCompare(app.appliance, "transportState", "Stopped", 500)
        compare(app.appliance.machineError, "AudioOutputFailure")
    }
}
