import QtQuick
import QtQuick3D

Item {
    id: surface
    required property var appliance
    required property var sceneRoot
    readonly property var lcdModel: resolveScreen()
    readonly property bool assetReady: lcdModel !== null
    readonly property string assetError: assetReady ? "" : "LCD asset contract failure"
    readonly property alias plane: screenPlane
    readonly property alias display: content

    function unique(root, name) {
        let match = null
        let count = 0
        function visit(node) {
            if (!node)
                return
            if (node.objectName === name) { match = node; count++ }
            // Runtime texture/plane descendants must not participate in the
            // authored contract or create resolver binding dependencies.
            if (node instanceof Model)
                return
            for (const child of node.children)
                visit(child)
        }
        visit(root)
        return count === 1 ? match : null
    }
    function resolveScreen() {
        try {
            const player = unique(sceneRoot, "PLAYER_ROOT")
            const screen = unique(sceneRoot, "LCD_TEST")
            return player instanceof Node && !(player instanceof Model)
                && screen instanceof Model && screen.parent === player ? screen : null
        } catch (error) { return null }
    }
    Model {
        id: screenPlane
        parent: surface.lcdModel
        visible: surface.assetReady
        source: "#Rectangle"
        pickable: false
        // Built-in rectangle is 100 x 100. The authored root supplies the
        // existing metre-to-runtime scale; this plane stays in asset units.
        scale: Qt.vector3d(0.00045, 0.00006, 1)
        eulerRotation.x: -90
        position: Qt.vector3d(0, 0.00015, 0)
        materials: PrincipledMaterial {
            lighting: PrincipledMaterial.NoLighting
            alphaMode: PrincipledMaterial.Opaque
            baseColorMap: Texture {
                sourceItem: LcdDisplay {
                    id: content
                    appliance: surface.appliance
                }
            }
        }
    }
}
