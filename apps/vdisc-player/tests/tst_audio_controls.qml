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
        verify(app.audioTestControls !== undefined)
        tryCompare(app.contentItem, "width", app.width, 500)
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
    function high(slider) {
        slider.forceActiveFocus()
        for (let i = 0; i < 100; ++i)
            keyClick(Qt.Key_Right)
    }
    function test_keyboard_and_mouse_change_live_session_gain() {
        const app = setup(true)
        const slider = app.audioTestControls.volumeSlider
        slider.forceActiveFocus(); keyClick(Qt.Key_Right)
        fuzzyCompare(app.appliance.volume, 0.51, 0.00001)
        fuzzyCompare(audioTestDriver.gain(), 0.51, 0.00001)
        mouseClick(slider, slider.width * 0.25, slider.height / 2)
        verify(app.appliance.volume < 0.4)
        fuzzyCompare(audioTestDriver.gain(), app.appliance.volume, 0.00001)
        slider.forceActiveFocus()
        for (let i = 0; i < 100; ++i)
            keyClick(Qt.Key_Left)
        compare(app.appliance.volume, 0)
        compare(audioTestDriver.gain(), 0)
    }
    function test_repeated_avls_clamp_restores_slider_without_notify() {
        const app = setup(true)
        const panel = app.audioTestControls
        high(panel.volumeSlider)
        mouseClick(panel.avlsButton)
        verify(app.appliance.avlsEnabled)
        fuzzyCompare(panel.volumeSlider.value, 0.75, 0.00001)
        fuzzyCompare(audioTestDriver.gain(), 0.75, 0.00001)
        for (let i = 0; i < 3; ++i) {
            high(panel.volumeSlider)
            fuzzyCompare(panel.volumeSlider.value, 0.75, 0.00001)
            fuzzyCompare(app.appliance.volume, 0.75, 0.00001)
        }
        mouseClick(panel.avlsButton)
        verify(!app.appliance.avlsEnabled)
        fuzzyCompare(panel.volumeSlider.value, 0.75, 0.00001)
    }
    function test_hold_allows_volume_rejects_avls_and_restores_toggle() {
        const app = setup(false)
        verify(app.appliance.setHold(true))
        high(app.audioTestControls.volumeSlider)
        compare(app.appliance.volume, 1)
        mouseClick(app.audioTestControls.avlsButton)
        verify(!app.appliance.avlsEnabled)
        verify(!app.audioTestControls.avlsButton.checked)
        verify(app.appliance.lastRejection.length > 0)
    }
    function test_real_file_dialog_disables_panel_and_outside_controls_work() {
        const app = setup(false)
        compare(app.audioTestControls.width, 280)
        verify(app.audioTestControls.y > 60)
        mouseClick(app.discDeck, 70, 35)
        tryCompare(app.discDeck, "filePickerVisible", true, 500)
        verify(!app.audioTestControls.enabled)
        // QtTest key events target its own window, not the modal native dialog.
        // Close the real dialog object after verifying its production visibility binding.
        let dialog = null
        for (const object of app.discDeck.data) {
            if (object.title === "Choose a VDISC")
                dialog = object
        }
        verify(dialog !== null)
        dialog.close()
        tryCompare(app.discDeck, "filePickerVisible", false, 500)
        verify(app.audioTestControls.enabled)
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
