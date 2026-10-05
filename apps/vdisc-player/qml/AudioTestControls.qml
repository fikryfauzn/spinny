import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    required property var appliance
    required property bool filePickerVisible
    readonly property alias volumeSlider: volumeSlider
    readonly property alias avlsButton: avlsButton
    width: Math.min(280, parent ? parent.width - 32 : 280)
    height: content.implicitHeight + 24
    radius: 5
    color: "#35414c"
    border.color: "#a8b9c5"
    enabled: !filePickerVisible

    // Only this compact panel absorbs scene gestures, including its empty margin.
    MouseArea { anchors.fill: parent }

    ColumnLayout {
        id: content
        anchors.fill: parent
        anchors.margins: 12
        spacing: 4
        Label { text: "Audio test controls"; color: "white" }
        Slider {
            id: volumeSlider
            Layout.fillWidth: true
            from: 0
            to: 1
            stepSize: 0.01
            value: root.appliance.volume
            onMoved: {
                root.appliance.setVolume(value)
                value = Qt.binding(function() { return root.appliance.volume })
            }
        }
        RowLayout {
            Button {
                id: avlsButton
                text: "AVLS " + (root.appliance.avlsEnabled ? "ON" : "OFF")
                checkable: true
                checked: root.appliance.avlsEnabled
                onClicked: {
                    root.appliance.requestToggleAvls()
                    checked = Qt.binding(function() { return root.appliance.avlsEnabled })
                }
            }
            Label {
                text: "Gain " + root.appliance.applicationGain.toFixed(2)
                color: "white"
            }
        }
    }
}
