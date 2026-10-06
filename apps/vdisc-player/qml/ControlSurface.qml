import QtQuick
import QtQuick3D

Item {
    id: controls
    required property var view3d
    required property var appliance
    required property var sceneRoot
    property bool windowActive: true
    property bool filePickerVisible: false
    property int inspectionRevision: 0
    readonly property bool inputActive: windowActive && !filePickerVisible
    readonly property var names: ["BTN_PLAY_TEST", "BTN_PAUSE_TEST", "BTN_STOP_TEST",
                                 "BTN_PREVIOUS_TEST", "BTN_NEXT_TEST", "BTN_OPEN_TEST", "BTN_MENU_TEST"]
    readonly property var handles: names.map(function(name) { return resolve(name) })
    readonly property var railNames: ["HOLD_RAIL_TEST", "HOLD_KNOB_TEST", "VOLUME_RAIL_TEST", "VOLUME_KNOB_TEST"]
    readonly property var railHandles: railNames.map(function(name) { return resolve(name) })
    readonly property bool assetReady: handles.every(function(model) {
        return !!model && model instanceof Model && model.parent && model.parent.objectName === "PLAYER_ROOT"
               && model.parent === resolve("PLAYER_ROOT")
    }) && railHandles.every(function(model) {
        return !!model && model instanceof Model && model.parent === resolve("PLAYER_ROOT")
    })
    readonly property alias gesture: gesture
    readonly property alias holdRail: holdRail
    readonly property alias volumeRail: volumeRail
    property var capturedRail: null
    readonly property string captureKind: capturedIndex >= 0 ? "Button" : capturedRail ? capturedRail.kind : ""
    property int capturedIndex: -1
    property Model capturedModel: null
    property string hoveredName: ""
    readonly property string pressedName: capturedIndex < 0 ? "" : names[capturedIndex]
    readonly property string hintText: !assetReady ? "Control asset contract failure"
        : capturedRail ? capturedRail.hintText : pressedName !== "" ? label(pressedName)
        : hoveredName.startsWith("HOLD_") ? holdRail.hintText
        : hoveredName.startsWith("VOLUME_") ? volumeRail.hintText : label(hoveredName)
    readonly property var travels: [playTravel, pauseTravel, stopTravel,
                                   previousTravel, nextTravel, openTravel, menuTravel]

    function resolve(name) {
        let match = null
        let count = 0
        function visit(node) {
            if (node.objectName === name) {
                match = node
                count++
            }
            if (!(node instanceof Model)) {
                for (const child of node.children)
                    visit(child)
            }
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
        if (!inputActive || !assetReady || captureKind !== "" || !hitObject || !(hitObject instanceof Model))
            return false
        const index = handles.indexOf(hitObject)
        if (index < 0 || !travels[index].press())
            return false
        capturedIndex = index
        capturedModel = hitObject
        gesture.begin(names[index])
        return true
    }
    function releaseTravel() {
        const index = capturedIndex
        capturedIndex = -1
        capturedModel = null
        if (index >= 0)
            travels[index].release()
    }
    function cancelPress() {
        releaseTravel()
        gesture.cancel()
        const rail = capturedRail
        capturedRail = null
        if (rail) rail.cancel()
    }
    function beginRail(hitObject, x, y) {
        if (!inputActive || !assetReady || captureKind !== "") return false
        const index = railHandles.indexOf(hitObject)
        if (index < 0) return false
        const rail = index < 2 ? holdRail : volumeRail
        if (!rail.begin(x, y, index % 2 === 1)) return false
        capturedRail = rail
        return true
    }
    function finishRail(x, y) {
        const rail = capturedRail
        capturedRail = null
        return !!rail && rail.finish(x, y)
    }
    function finishPress(hitObject) {
        const index = capturedIndex
        const valid = index >= 0 && inputActive && assetReady && hitObject === capturedModel
                      && capturedModel === handles[index]
        releaseTravel()
        return gesture.finish(valid)
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
    onRailHandlesChanged: cancelPress()
    onInputActiveChanged: {
        if (!inputActive)
            cancelPress()
    }
    onInspectionRevisionChanged: cancelPress()
    Component.onDestruction: cancelPress()
    Shortcut { sequence: "Escape"; onActivated: controls.cancelPress() }
    ButtonGesture {
        id: gesture
        appliance: controls.appliance
        nowMs: function() { return controls.appliance.inputTimeMs() }
        onHeldChanged: { if (!held) controls.releaseTravel() }
    }
    RailControl {
        id: holdRail
        appliance: controls.appliance
        view3d: controls.view3d
        railModel: controls.railHandles[0]
        knobModel: controls.railHandles[1]
        kind: "Hold"
        onActiveChanged: { if (!active && controls.capturedRail === holdRail) controls.capturedRail = null }
    }
    RailControl {
        id: volumeRail
        appliance: controls.appliance
        view3d: controls.view3d
        railModel: controls.railHandles[2]
        knobModel: controls.railHandles[3]
        kind: "Volume"
        onActiveChanged: { if (!active && controls.capturedRail === volumeRail) controls.capturedRail = null }
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
    ButtonTravel {
        id: menuTravel
        model: controls.assetReady ? controls.handles[6] : null
        travelOffset: Qt.vector3d(0, 0, -0.0006)
    }
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        hoverEnabled: true
        onPressed: function(mouse) {
            const hit = controls.hitAt(mouse.x, mouse.y)
            mouse.accepted = controls.beginPress(hit) || controls.beginRail(hit, mouse.x, mouse.y)
        }
        onReleased: function(mouse) {
            if (controls.capturedRail) controls.finishRail(mouse.x, mouse.y)
            else controls.finishPress(controls.hitAt(mouse.x, mouse.y))
        }
        onCanceled: controls.cancelPress()
        onPositionChanged: function(mouse) {
            if (controls.capturedRail) {
                controls.capturedRail.update(mouse.x, mouse.y)
                return
            }
            const hit = controls.hitAt(mouse.x, mouse.y)
            if (controls.capturedModel && hit !== controls.capturedModel)
                controls.cancelPress()
            const index = controls.handles.indexOf(hit)
            const railIndex = controls.railHandles.indexOf(hit)
            controls.hoveredName = index >= 0 ? controls.names[index] : railIndex >= 0 ? controls.railNames[railIndex] : ""
        }
        onExited: { controls.hoveredName = ""; if (!controls.capturedRail) controls.cancelPress() }
    }
    Text {
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottomMargin: 16
        text: controls.hintText
        color: "#e1e9ee"
    }
}
