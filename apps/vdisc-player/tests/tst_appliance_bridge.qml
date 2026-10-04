import QtQuick
import QtTest
import VdiscAppliance 1.0
import "../qml" as App

TestCase {
    name: "ApplianceBridge"

    Component {
        id: bridgeComponent
        ApplianceBridge {}
    }

    Component {
        id: spyComponent
        SignalSpy { signalName: "lidStateChanged" }
    }

    Component {
        id: appComponent
        App.Main {}
    }

    function test_main_has_one_appliance_bridge() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const bridge = app.appliance
            verify(bridge !== undefined && bridge !== null)
            compare(bridge.lidState, "Closed")
            verify(bridge.requestOpen())
            compare(bridge.lidState, "Opening")
        } finally {
            app.destroy()
        }
    }

    function test_open_round_trip_and_rejection() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        const spy = spyComponent.createObject(null, { target: bridge })
        verify(spy !== null)
        try {
            compare(bridge.lidState, "Closed")
            verify(bridge.requestOpen())
            compare(bridge.lidState, "Opening")
            compare(spy.count, 1)
            verify(bridge.lidOpened())
            compare(bridge.lidState, "Open")
            compare(spy.count, 2)
            verify(!bridge.requestOpen())
            compare(bridge.lidState, "Open")
            verify(bridge.lastRejection.length > 0)
            compare(spy.count, 2)
        } finally {
            spy.destroy()
            bridge.destroy()
        }
    }

    function test_projection_cannot_be_set_by_qml() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        try {
            try {
                bridge.lidState = "Open"
            } catch (error) {
                // A read-only property may throw on assignment.
            }
            compare(bridge.lidState, "Closed")
        } finally {
            bridge.destroy()
        }
    }

    function test_invalid_numeric_requests_are_rejected() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        try {
            compare(bridge.volume, 0.5)
            verify(!bridge.setVolume(NaN))
            compare(bridge.volume, 0.5)
            verify(!bridge.requestScanStep(-1))
            compare(bridge.transportState, "Stopped")
        } finally {
            bridge.destroy()
        }
    }

    function test_empty_appliance_rejects_transport_and_disc_actions() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        try {
            verify(!bridge.requestClose())
            verify(!bridge.requestInsert(""))
            verify(!bridge.requestRemove())
            verify(!bridge.requestPlay())
            verify(!bridge.requestPause())
            verify(!bridge.requestStop())
            verify(!bridge.requestPrevious())
            verify(!bridge.requestNext())
            verify(!bridge.requestScanBegin(true))
            verify(!bridge.requestScanEnd())
            verify(!bridge.requestScanStep(1))
            verify(!bridge.requestMenuShort())
            verify(!bridge.lidClosed())
            verify(!bridge.discInserted())
            verify(!bridge.discRemoved())
            compare(bridge.lidState, "Closed")
            compare(bridge.discState, "Absent")
            compare(bridge.transportState, "Stopped")
            verify(bridge.lastRejection.length > 0)
        } finally {
            bridge.destroy()
        }
    }

    function test_hold_avls_volume_and_lcd_projection() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        try {
            verify(bridge.setVolume(0.7))
            compare(bridge.volume, 0.7)
            compare(bridge.applicationGain, 0.7)
            verify(bridge.requestMenuLong())
            compare(bridge.avlsEnabled, true)
            compare(bridge.lcdAvls, true)
            verify(bridge.requestToggleAvls())
            compare(bridge.avlsEnabled, false)
            verify(bridge.setHold(true))
            compare(bridge.holdEnabled, true)
            compare(bridge.lcdHold, true)
            verify(!bridge.requestOpen())
            verify(bridge.setHold(false))
            compare(bridge.holdEnabled, false)
            compare(bridge.lcdTrackNumber, -1)
            compare(bridge.lcdElapsedMs, -1)
            compare(bridge.lcdTotalTracks, -1)
            compare(bridge.lcdTotalMs, -1)
            compare(bridge.lcdPlayMode, "Normal")
            compare(bridge.lcdPlaybackStatus, "Stopped")
            compare(bridge.lcdMessage, "")
            compare(bridge.machineError, "")
        } finally {
            bridge.destroy()
        }
    }

    function test_valid_disc_uses_core_validation_and_completion() {
        const bridge = bridgeComponent.createObject(null)
        verify(bridge !== null)
        try {
            verify(bridge.requestOpen())
            verify(bridge.lidOpened())
            const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
            const path = decodeURIComponent(url.substring("file://".length))
            verify(bridge.requestInsert(path), bridge.lastRejection)
            compare(bridge.discState, "Inserting")
            compare(bridge.lcdTrackNumber, 1)
            verify(bridge.discInserted())
            compare(bridge.discState, "Seated")
            verify(bridge.requestRemove())
            compare(bridge.discState, "Removing")
            verify(bridge.discRemoved())
            compare(bridge.discState, "Absent")
        } finally {
            bridge.destroy()
        }
    }
}
