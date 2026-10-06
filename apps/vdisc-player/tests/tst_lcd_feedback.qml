import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "LcdFeedback"
    when: windowShown
    Component { id: appComponent; App.Main {} }
    Component { id: timerComponent; App.LcdFeedbackRuntime {} }
    QtObject {
        id: stub
        property bool lcdFeedbackActive: false
        property int refreshes: 0
        function refreshLcdFeedback() { refreshes++ }
    }
    function test_wake_only_when_active() {
        stub.lcdFeedbackActive = false; stub.refreshes = 0
        const timer = createTemporaryObject(timerComponent, this, {appliance: stub})
        verify(timer !== null)
        wait(120); compare(stub.refreshes, 0)
        stub.lcdFeedbackActive = true
        tryVerify(function() { return stub.refreshes >= 2 }, 300)
        stub.lcdFeedbackActive = false
        const count = stub.refreshes
        wait(120); compare(stub.refreshes, count)
    }
    function test_stopped_hidden_expiry_preserves_diagnostics() {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        const a = app.appliance
        verify(a.setHold(true)); verify(!a.requestMenuShort())
        compare(a.lcdMessage, "Hold"); compare(a.transportState, "Stopped")
        verify(a.lcdFeedbackActive)
        const rejection = a.lastRejection
        app.visible = false
        tryCompare(a, "lcdMessage", "", 2000)
        compare(a.lcdFeedbackActive, false)
        compare(a.lastRejection, rejection)
        compare(a.lcdHold, true)
        verify(!app.lcdFeedbackRuntime.feedbackTimer.running)
    }
    function test_repeat_then_unlock() {
        const app = createTemporaryObject(appComponent, null)
        const a = app.appliance
        verify(a.setHold(true)); verify(!a.requestToggleAvls())
        wait(1000)
        verify(!a.requestMenuLong())
        wait(650); compare(a.lcdMessage, "Hold")
        tryCompare(a, "lcdMessage", "", 1300)
        verify(!a.requestMenuShort()); compare(a.lcdMessage, "Hold")
        verify(a.setHold(false)); compare(a.lcdMessage, "")
        compare(a.lcdFeedbackActive, false)
    }
    function test_refresh_does_not_clear_machine_error_or_rejection() {
        const app = createTemporaryObject(appComponent, null)
        const a = app.appliance
        verify(a.requestOpen()); tryCompare(a, "lidState", "Open", 1000)
        verify(!a.requestInsert("/missing-lcd-disc.vdisc"))
        verify(a.machineError.length > 0)
        const error = a.machineError; const rejection = a.lastRejection
        a.refreshLcdFeedback()
        compare(a.machineError, error); compare(a.lastRejection, rejection)
    }
}
