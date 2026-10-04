import QtQuick
import QtQuick.Window
import QtQuick3D
import "../generated/phase03-greybox" as Greybox

Window {
    width: 960
    height: 720
    visible: true
    title: "VDISC Phase 3 Greybox"
    readonly property alias greybox: greyboxScene

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
            id: greyboxScene
        }
    }
}
