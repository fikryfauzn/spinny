import QtQuick

Item {
    id: feedback
    required property var appliance
    readonly property alias feedbackTimer: wake
    Timer {
        id: wake
        interval: 50
        repeat: true
        running: feedback.appliance.lcdFeedbackActive
        onTriggered: feedback.appliance.refreshLcdFeedback()
    }
}
