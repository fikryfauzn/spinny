import QtQuick

Item {
    id: root
    required property var appliance
    readonly property alias pollingTimer: backendTimer

    Timer {
        id: backendTimer
        interval: 20
        repeat: true
        running: ["Playing", "Paused", "SeekingForward", "SeekingBackward"].indexOf(root.appliance.transportState) >= 0
        onTriggered: root.appliance.pollBackend()
    }
}
