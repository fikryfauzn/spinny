import QtQuick
import QtQuick3D

Item {
    id: inspection
    required property var sceneRoot
    required property var discModel
    required property bool windowActive
    required property bool filePickerVisible
    property bool cutawayEnabled: false
    readonly property var lidModel: resolveLid()
    readonly property bool assetReady: isLiveModel(lidModel)
    readonly property string assetError: assetReady ? "" : "Inspection asset contract failure"
    readonly property alias marker: witness
    readonly property alias shortcut: cutawayShortcut

    function isLiveModel(object) {
        try { return !!object && object instanceof Model }
        catch (error) { return false }
    }
    function unique(root, name) {
        let match = null
        let count = 0
        function visit(node) {
            if (!node)
                return
            if (node.objectName === name) { match = node; count++ }
            // The authored contract has no Model children; ignore runtime witnesses.
            if (node instanceof Model)
                return
            for (const child of node.children)
                visit(child)
        }
        visit(root)
        return count === 1 ? match : null
    }
    function resolveLid() {
        try {
            const root = unique(sceneRoot, "LID_ROOT")
            const model = unique(sceneRoot, "LID_TEST")
            return root instanceof Node && !(root instanceof Model)
                && isLiveModel(model) && model.parent === root ? model : null
        } catch (error) { return null }
    }
    function toggleCutaway() {
        if (!assetReady)
            return false
        cutawayEnabled = !cutawayEnabled
        return true
    }
    // Disabling the override before Binding destruction restores its saved value/binding.
    Component.onDestruction: cutawayEnabled = false

    Model {
        id: witness
        parent: inspection.isLiveModel(inspection.discModel) ? inspection.discModel : null
        visible: inspection.isLiveModel(inspection.discModel)
        source: "#Cube"
        position: Qt.vector3d(0.0325, 0.0007, 0)
        // Built-in cube has 100-unit sides; authored scene units are meters.
        scale: Qt.vector3d(0.0004, 0.000001, 0.000015)
        pickable: false
        materials: DefaultMaterial {
            lighting: DefaultMaterial.NoLighting
            diffuseColor: "#18232d"
        }
    }
    Binding {
        target: inspection.lidModel
        property: "visible"
        value: false
        when: inspection.assetReady && inspection.cutawayEnabled
    }
    Shortcut {
        id: cutawayShortcut
        sequence: "V"
        autoRepeat: false
        enabled: inspection.assetReady && inspection.windowActive && !inspection.filePickerVisible
        onActivated: inspection.toggleCutaway()
    }
    Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 16
        width: 170
        height: 38
        radius: 5
        color: inspection.cutawayEnabled && inspection.assetReady ? "#506879" : "#35414c"
        border.color: "#a8b9c5"
        opacity: inspection.assetReady ? 1 : 0.5
        Text {
            anchors.centerIn: parent
            color: "white"
            text: "Cutaway [V]: " + (inspection.cutawayEnabled && inspection.assetReady ? "ON" : "OFF")
        }
        MouseArea {
            anchors.fill: parent
            enabled: inspection.assetReady && !inspection.filePickerVisible
            onClicked: inspection.toggleCutaway()
        }
    }
}
