import QtQuick
import QtQuick3D

Item {
    id: controls
    required property var view3d
    required property var appliance
    required property var sceneRoot
    property bool windowActive: true
    readonly property var names: ["BTN_PLAY_TEST", "BTN_PAUSE_TEST", "BTN_STOP_TEST",
                                 "BTN_PREVIOUS_TEST", "BTN_NEXT_TEST", "BTN_OPEN_TEST"]
    readonly property var handles: names.map(function(name) { return resolve(name) })
    readonly property bool assetReady: handles.every(function(model) {
        return !!model && model instanceof Model && model.parent && model.parent.objectName === "PLAYER_ROOT"
               && model.parent === resolve("PLAYER_ROOT")
    })
    property int capturedIndex: -1
    property Model capturedModel: null
    property string hoveredName: ""
    readonly property string pressedName: capturedIndex < 0 ? "" : names[capturedIndex]
    readonly property string hintText: !assetReady ? "Control asset contract failure"
        : pressedName !== "" ? label(pressedName) : label(hoveredName)
    readonly property var travels: [playTravel, pauseTravel, stopTravel,
                                   previousTravel, nextTravel, openTravel]

    function resolve(name) {
        let match = null
        let count = 0
        function visit(node) {
            if (node.objectName === name) {
                match = node
                count++
            }
            for (const child of node.children)
                visit(child)
        }
        if (sceneRoot)
            visit(sceneRoot)
        return count === 1 ? match : null
    }
    function label(name) {
        return name === "" ? "" : name.replace("BTN_", "").replace("_TEST", "")
    }
    function buttonForName(name) {
        const index = names.indexOf(name)
        return assetReady && index >= 0 ? handles[index] : null
    }
    function beginPress(hitObject) {
        if (!assetReady || capturedIndex !== -1 || !hitObject || !(hitObject instanceof Model))
            return false
        const index = handles.indexOf(hitObject)
        if (index < 0 || !travels[index].press())
            return false
        capturedIndex = index
        capturedModel = hitObject
        return true
    }
    function cancelPress() {
        const index = capturedIndex
        capturedIndex = -1
        capturedModel = null
        if (index >= 0)
            travels[index].release()
    }
    function finishPress(hitObject) {
        const index = capturedIndex
        const valid = index >= 0 && assetReady && hitObject === capturedModel
                      && capturedModel === handles[index]
        cancelPress()
        if (!valid)
            return false
        switch (names[index]) {
        case "BTN_PLAY_TEST": return appliance.requestPlay()
        case "BTN_PAUSE_TEST": return appliance.requestPause()
        case "BTN_STOP_TEST": return appliance.requestStop()
        case "BTN_PREVIOUS_TEST": return appliance.requestPrevious()
        case "BTN_NEXT_TEST": return appliance.requestNext()
        case "BTN_OPEN_TEST":
            return appliance.lidState === "Open" ? appliance.requestClose() : appliance.requestOpen()
        }
        return false
    }
    function hitAt(x, y) {
        return assetReady && view3d && view3d.camera ? view3d.pick(x, y).objectHit : null
    }
    onAssetReadyChanged: {
        if (!assetReady) {
            cancelPress()
            hoveredName = ""
        }
    }
    onHandlesChanged: {
        if (capturedIndex >= 0 && capturedModel !== handles[capturedIndex])
            cancelPress()
    }
    onWindowActiveChanged: {
        if (!windowActive)
            cancelPress()
    }

    ButtonTravel {
        id: playTravel
        model: controls.assetReady ? controls.handles[0] : null
        travelOffset: Qt.vector3d(0, -0.0006, 0)
    }
    ButtonTravel {
        id: pauseTravel
        model: controls.assetReady ? controls.handles[1] : null
        travelOffset: Qt.vector3d(0, -0.0006, 0)
    }
    ButtonTravel {
        id: stopTravel
        model: controls.assetReady ? controls.handles[2] : null
        travelOffset: Qt.vector3d(0, -0.0006, 0)
    }
    ButtonTravel {
        id: previousTravel
        model: controls.assetReady ? controls.handles[3] : null
        travelOffset: Qt.vector3d(0, 0, -0.0006)
    }
    ButtonTravel {
        id: nextTravel
        model: controls.assetReady ? controls.handles[4] : null
        travelOffset: Qt.vector3d(0, 0, -0.0006)
    }
    ButtonTravel {
        id: openTravel
        model: controls.assetReady ? controls.handles[5] : null
        travelOffset: Qt.vector3d(0, -0.0006, 0)
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        hoverEnabled: true
        onPressed: function(mouse) {
            mouse.accepted = controls.beginPress(controls.hitAt(mouse.x, mouse.y))
        }
        onReleased: function(mouse) {
            controls.finishPress(controls.hitAt(mouse.x, mouse.y))
        }
        onCanceled: controls.cancelPress()
        onPositionChanged: function(mouse) {
            const hit = controls.hitAt(mouse.x, mouse.y)
            const index = controls.handles.indexOf(hit)
            controls.hoveredName = index >= 0 ? controls.names[index] : ""
        }
        onExited: controls.hoveredName = ""
    }
    Text {
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 16
        text: controls.hintText
        color: "#e1e9ee"
    }
}
