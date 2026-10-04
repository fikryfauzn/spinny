import QtQuick
import QtQuick.Window
import QtQuick3D
import "../generated/phase03-greybox" as Greybox

Window {
    id: root
    width: 960
    height: 720
    visible: true
    title: "VDISC Open-Lid Asset Preview"

    function findNode(node, name) {
        if (node.objectName === name)
            return node
        for (let child of node.children) {
            const match = findNode(child, name)
            if (match)
                return match
        }
        return null
    }

    View3D {
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
            id: greybox
            Component.onCompleted: {
                const lid = root.findNode(greybox, "LID_ROOT")
                if (lid)
                    lid.eulerRotation = Qt.vector3d(-65, 0, 0)
            }
        }
    }
}
