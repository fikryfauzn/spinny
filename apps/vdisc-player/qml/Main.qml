import QtQuick
import QtQuick.Window
import QtQuick.Dialogs
import VdiscAppliance 1.0

Window {
    id: window
    width: 960
    height: 720
    minimumWidth: 760
    minimumHeight: 600
    visible: true
    title: "VDISC Player"
    color: "#171d24"

    readonly property alias appliance: applianceBridge
    readonly property alias audioRuntime: audioRuntime
    readonly property alias lcdFeedbackRuntime: lcdFeedbackRuntime
    property string selectedPath: ""
    readonly property bool discReady: applianceBridge.discState === "Seated"
    readonly property bool lidOpen: applianceBridge.lidState === "Open"
    readonly property string statusText: applianceBridge.machineError !== "" ? applianceBridge.machineError
        : applianceBridge.lastRejection !== "" ? applianceBridge.lastRejection
        : applianceBridge.insertionPending ? "Verifying disc…"
        : selectedPath !== "" ? selectedPath : "Choose a .vdisc file to begin"

    function selectDiscUrl(url) {
        if (applianceBridge.discState !== "Absent")
            return false
        const value = String(url)
        if (!value.startsWith("file://"))
            return false
        selectedPath = decodeURIComponent(value.substring("file://".length))
        return true
    }

    function insertDisc() {
        if (!lidOpen || selectedPath === "" || applianceBridge.discState !== "Absent")
            return false
        if (!applianceBridge.requestInsert(selectedPath))
            return false
        applianceBridge.discInserted()
        return true
    }

    function removeDisc() {
        if (!lidOpen || !discReady || !applianceBridge.requestRemove())
            return false
        applianceBridge.discRemoved()
        return true
    }

    ApplianceBridge { id: applianceBridge }
    AudioRuntime { id: audioRuntime; appliance: applianceBridge }
    LcdFeedbackRuntime { id: lcdFeedbackRuntime; appliance: applianceBridge }

    Timer {
        interval: 32
        repeat: true
        running: applianceBridge.insertionPending
        onTriggered: applianceBridge.pollInsertion()
    }
    Timer {
        id: lidTimer
        interval: 250
        onTriggered: {
            if (applianceBridge.lidState === "Opening") applianceBridge.lidOpened()
            else if (applianceBridge.lidState === "Closing") applianceBridge.lidClosed()
        }
    }
    Connections {
        target: applianceBridge
        function onLidStateChanged() {
            if (applianceBridge.lidState === "Opening" || applianceBridge.lidState === "Closing")
                lidTimer.restart()
        }
    }
    Shortcut { sequence: "O"; onActivated: applianceBridge.requestOpen() }
    Shortcut { sequence: "C"; onActivated: applianceBridge.requestClose() }

    ButtonGesture {
        id: gesture
        appliance: applianceBridge
        nowMs: function() { return applianceBridge.inputTimeMs() }
    }

    FileDialog {
        id: discFileDialog
        title: "Choose a VDISC"
        nameFilters: ["VDISC discs (*.vdisc)"]
        onAccepted: window.selectDiscUrl(selectedFile)
    }

    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(parent.width - 40, 760)
        height: Math.min(parent.height - 40, 640)
        radius: 22
        color: "#29323a"
        border.color: "#58636a"
        border.width: 2

        Column {
            anchors.fill: parent
            anchors.margins: 32
            spacing: 20

            Text {
                text: "VDISC PLAYER"
                color: "#dce5e4"
                font.pixelSize: 22
                font.bold: true
                font.letterSpacing: 3
            }

            LcdDisplay {
                width: parent.width
                height: 120
                appliance: applianceBridge
            }

            Text {
                width: parent.width
                height: 44
                text: window.statusText
                color: applianceBridge.machineError !== "" || applianceBridge.lastRejection !== ""
                       ? "#ffc7ae" : "#dce5e4"
                elide: Text.ElideMiddle
                wrapMode: Text.Wrap
                font.pixelSize: 15
            }

            Row {
                spacing: 12
                Repeater {
                    model: [
                        { label: "CHOOSE DISC", action: "choose" },
                        { label: window.lidOpen ? "CLOSE LID" : "OPEN LID", action: "lid" },
                        { label: "INSERT", action: "insert" },
                        { label: "REMOVE", action: "remove" }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 155; height: 46; radius: 8
                        color: actionMouse.pressed ? "#b9d3d1" : "#3c535c"
                        border.color: "#8aa5a9"
                        Text { anchors.centerIn: parent; text: modelData.label; color: "white"; font.bold: true }
                        MouseArea {
                            id: actionMouse
                            anchors.fill: parent
                            onClicked: {
                                switch (modelData.action) {
                                case "choose": if (applianceBridge.discState === "Absent") discFileDialog.open(); break
                                case "lid": if (window.lidOpen) applianceBridge.requestClose(); else applianceBridge.requestOpen(); break
                                case "insert": window.insertDisc(); break
                                case "remove": window.removeDisc(); break
                                }
                            }
                        }
                    }
                }
            }

            Text { text: "TRANSPORT"; color: "#a9b9ba"; font.pixelSize: 13; font.bold: true }
            Row {
                spacing: 12
                Repeater {
                    model: [
                        { label: "◀◀", name: "BTN_PREVIOUS_TEST" },
                        { label: "PLAY", name: "BTN_PLAY_TEST" },
                        { label: "PAUSE", name: "BTN_PAUSE_TEST" },
                        { label: "STOP", name: "BTN_STOP_TEST" },
                        { label: "▶▶", name: "BTN_NEXT_TEST" },
                        { label: "MODE", name: "BTN_MENU_TEST" }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 98; height: 64; radius: 8
                        color: controlMouse.pressed ? "#b9d3d1" : "#4c636b"
                        border.color: "#a2b8ba"
                        Text { anchors.centerIn: parent; text: modelData.label; color: "white"; font.bold: true }
                        MouseArea {
                            id: controlMouse
                            anchors.fill: parent
                            onPressed: gesture.begin(modelData.name)
                            onReleased: gesture.finish(containsMouse)
                            onCanceled: gesture.cancel()
                        }
                    }
                }
            }

            Row {
                spacing: 22
                Text { text: "HOLD"; color: "#dce5e4"; anchors.verticalCenter: parent.verticalCenter }
                Rectangle {
                    width: 62; height: 32; radius: 16
                    color: applianceBridge.holdEnabled ? "#d5a77e" : "#657980"
                    Rectangle {
                        x: applianceBridge.holdEnabled ? 32 : 2
                        y: 2; width: 28; height: 28; radius: 14; color: "white"
                    }
                    MouseArea { anchors.fill: parent; onClicked: applianceBridge.setHold(!applianceBridge.holdEnabled) }
                }
                Text { text: "VOLUME"; color: "#dce5e4"; anchors.verticalCenter: parent.verticalCenter }
                Rectangle {
                    id: volumeTrack
                    width: 260; height: 12; radius: 6; color: "#17262e"
                    anchors.verticalCenter: parent.verticalCenter
                    Rectangle {
                        width: Math.max(0, Math.min(1, applianceBridge.volume)) * parent.width
                        height: parent.height; radius: 6; color: "#a9c9c5"
                    }
                    MouseArea {
                        anchors.fill: parent
                        anchors.topMargin: -14; anchors.bottomMargin: -14
                        onPressed: function(mouse) { applianceBridge.setVolume(Math.max(0, Math.min(1, mouse.x / volumeTrack.width))) }
                        onPositionChanged: function(mouse) {
                            if (pressed) applianceBridge.setVolume(Math.max(0, Math.min(1, mouse.x / volumeTrack.width)))
                        }
                    }
                }
                Text {
                    text: Math.round(applianceBridge.volume * 100) + "%"
                    color: "#dce5e4"
                    anchors.verticalCenter: parent.verticalCenter
                }
            }

            Text {
                text: "Lid: " + applianceBridge.lidState + "    Disc: " + applianceBridge.discState
                color: "#a9b9ba"
                font.pixelSize: 14
            }
        }
    }
}
