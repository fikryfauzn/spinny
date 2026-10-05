import QtQuick
import QtQuick.Window
import QtQuick3D
import VdiscAppliance 1.0
import "../generated/phase03-greybox" as Greybox

Window {
    width: 960
    height: 720
    visible: true
    title: "VDISC Phase 3 Greybox"
    readonly property alias greybox: greyboxScene
    readonly property alias appliance: applianceBridge
    readonly property var lidPivot: findUniqueNamedNode(greyboxScene, "LID_ROOT")
    readonly property alias discDeck: discDeck
    readonly property alias openingAnimation: openingMotion
    readonly property alias closingAnimation: closingMotion

    function findUniqueNamedNode(root, name) {
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

        if (root !== null)
            visit(root)
        return count === 1 ? match : null
    }

    ApplianceBridge {
        id: applianceBridge
    }

    Shortcut {
        sequence: "O"
        autoRepeat: false
        onActivated: applianceBridge.requestOpen()
    }

    Shortcut {
        sequence: "C"
        autoRepeat: false
        onActivated: applianceBridge.requestClose()
    }

    Connections {
        target: applianceBridge

        function onLidStateChanged() {
            if (applianceBridge.lidState === "Opening") {
                if (lidPivot)
                    openingMotion.start()
                else
                    console.error("Asset contract failure: unique LID_ROOT not found")
            } else if (applianceBridge.lidState === "Closing") {
                if (lidPivot)
                    closingMotion.start()
                else
                    console.error("Asset contract failure: unique LID_ROOT not found")
            }
        }
    }

    NumberAnimation {
        id: openingMotion
        target: lidPivot
        property: "eulerRotation.x"
        from: 0
        to: -105
        duration: 600
        easing.type: Easing.InOutQuad
        onFinished: {
            if (applianceBridge.lidState === "Opening")
                applianceBridge.lidOpened()
        }
    }

    NumberAnimation {
        id: closingMotion
        target: lidPivot
        property: "eulerRotation.x"
        from: -105
        to: 0
        duration: 600
        easing.type: Easing.InOutQuad
        onFinished: {
            if (applianceBridge.lidState === "Closing")
                applianceBridge.lidClosed()
        }
    }

    View3D {
        id: sceneView
        anchors.fill: parent

        environment: SceneEnvironment {
            backgroundMode: SceneEnvironment.Color
            clearColor: "#20252b"
        }

        camera: sceneCamera

        PerspectiveCamera {
            id: sceneCamera
            position: Qt.vector3d(20, 20, 36)
            eulerRotation: Qt.vector3d(-25, 29, 0)
            clipNear: 0.1
            clipFar: 200
        }

        DirectionalLight {
            eulerRotation: Qt.vector3d(-40, 25, 0)
        }

        Greybox.Phase03_greybox {
            id: greyboxScene
        }
    }

    DiscDeck {
        id: discDeck
        anchors.fill: sceneView
        view3d: sceneView
        appliance: applianceBridge
        discRoot: findUniqueNamedNode(greyboxScene, "DISC_ROOT")
        discModel: findUniqueNamedNode(greyboxScene, "DISC_TEST")
        spindleModel: findUniqueNamedNode(greyboxScene, "SPINDLE_TEST")
    }
}
