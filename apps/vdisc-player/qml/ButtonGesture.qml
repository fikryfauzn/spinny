import QtQuick

Item {
    id: gesture
    required property var appliance
    required property var nowMs
    property string name: ""
    property bool held: false
    property bool longConsumed: false
    property bool scanning: false
    property bool dispatching: false
    property bool longAccepted: false
    property real startedAt: 0
    readonly property alias longTimer: longDelay
    readonly property alias scanTimer: scanDelay

    function isSeeking() {
        return !!appliance && (appliance.transportState === "SeekingForward"
            || appliance.transportState === "SeekingBackward")
    }
    function hasLongAction() {
        return name === "BTN_MENU_TEST" || name === "BTN_PREVIOUS_TEST" || name === "BTN_NEXT_TEST"
    }
    function begin(buttonName) {
        if (held || ["BTN_PLAY_TEST", "BTN_PAUSE_TEST", "BTN_STOP_TEST", "BTN_OPEN_TEST",
                     "BTN_PREVIOUS_TEST", "BTN_NEXT_TEST", "BTN_MENU_TEST"].indexOf(buttonName) < 0)
            return false
        name = buttonName
        longConsumed = false
        longAccepted = false
        scanning = false
        startedAt = nowMs()
        held = true
        if (hasLongAction()) {
            longDelay.interval = 600
            longDelay.start()
        }
        return true
    }
    function consumeLong() {
        if (!held || longConsumed || !hasLongAction())
            return false
        const remaining = 600 - (nowMs() - startedAt)
        if (remaining > 0) {
            longDelay.interval = remaining
            longDelay.restart()
            return false
        }
        longDelay.stop()
        longConsumed = true
        dispatching = true
        const accepted = name === "BTN_MENU_TEST" ? appliance.requestMenuLong()
            : appliance.requestScanBegin(name === "BTN_NEXT_TEST")
        dispatching = false
        longAccepted = accepted
        scanning = accepted && name !== "BTN_MENU_TEST" && isSeeking()
        if (scanning)
            scanDelay.start()
        if (appliance.holdEnabled && scanning)
            cancel()
        return accepted
    }
    function scanTick() {
        if (!held || !scanning)
            return false
        const accepted = appliance.requestScanRelative(name === "BTN_NEXT_TEST" ? 1600 : -1600)
        if (!accepted)
            cancel()
        return accepted
    }
    function clear() {
        longDelay.stop()
        scanDelay.stop()
        scanning = false
        held = false
    }
    function cancel() {
        const endScan = scanning && isSeeking()
        clear()
        if (endScan)
            appliance.requestScanEnd()
    }
    function finish(validRelease) {
        if (!held)
            return false
        if (!validRelease) {
            cancel()
            return false
        }
        if (hasLongAction() && !longConsumed && nowMs() - startedAt >= 600)
            consumeLong()
        if (!held)
            return false
        const buttonName = name
        const consumed = longConsumed
        const accepted = longAccepted
        const endScan = scanning && isSeeking()
        clear()
        if (endScan)
            appliance.requestScanEnd()
        if (consumed)
            return accepted
        switch (buttonName) {
        case "BTN_PLAY_TEST": return appliance.requestPlay()
        case "BTN_PAUSE_TEST": return appliance.requestPause()
        case "BTN_STOP_TEST": return appliance.requestStop()
        case "BTN_PREVIOUS_TEST": return appliance.requestPrevious()
        case "BTN_NEXT_TEST": return appliance.requestNext()
        case "BTN_MENU_TEST": return appliance.requestMenuShort()
        case "BTN_OPEN_TEST":
            return appliance.lidState === "Open" ? appliance.requestClose() : appliance.requestOpen()
        }
        return false
    }
    Connections {
        target: gesture.appliance
        function onHoldEnabledChanged() {
            if (!gesture.dispatching && gesture.appliance.holdEnabled)
                gesture.cancel()
        }
        function onTransportStateChanged() {
            if (!gesture.dispatching && gesture.scanning && !gesture.isSeeking())
                gesture.cancel()
        }
    }
    Timer { id: longDelay; interval: 600; onTriggered: gesture.consumeLong() }
    Timer { id: scanDelay; interval: 200; repeat: true; onTriggered: gesture.scanTick() }
    Component.onDestruction: cancel()
}
