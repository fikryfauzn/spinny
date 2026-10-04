import QtQuick
import QtQuick3D

Node {
    id: node
    scale: Qt.vector3d(100, 100, 100)

    // Resources
    PrincipledMaterial {
        id: grey_BODY_material
        objectName: "GREY_BODY"
        baseColor: "#ffa0a8b1"
        roughness: 0.5
        cullMode: PrincipledMaterial.NoCulling
        alphaMode: PrincipledMaterial.Opaque
    }
    PrincipledMaterial {
        id: grey_BUTTON_material
        objectName: "GREY_BUTTON"
        baseColor: "#ff6c737b"
        roughness: 0.5
        cullMode: PrincipledMaterial.NoCulling
        alphaMode: PrincipledMaterial.Opaque
    }
    PrincipledMaterial {
        id: grey_DISC_material
        objectName: "GREY_DISC"
        baseColor: "#ffdce2e6"
        roughness: 0.5
        cullMode: PrincipledMaterial.NoCulling
        alphaMode: PrincipledMaterial.Opaque
    }
    PrincipledMaterial {
        id: grey_LCD_material
        objectName: "GREY_LCD"
        baseColor: "#ff4b6169"
        roughness: 0.5
        cullMode: PrincipledMaterial.NoCulling
        alphaMode: PrincipledMaterial.Opaque
    }
    PrincipledMaterial {
        id: grey_LID_material
        objectName: "GREY_LID"
        baseColor: "#ffc3cbd3"
        roughness: 0.5
        cullMode: PrincipledMaterial.NoCulling
        alphaMode: PrincipledMaterial.Opaque
    }

    // Nodes:
    Node {
        id: player_ROOT
        objectName: "PLAYER_ROOT"
        Model {
            id: body_TEST
            objectName: "BODY_TEST"
            source: "meshes/body_TEST_MESH_mesh.mesh"
            materials: [
                grey_BODY_material
            ]
        }
        Model {
            id: btn_OPEN_TEST
            objectName: "BTN_OPEN_TEST"
            position: Qt.vector3d(-0.003, 0.0235, 0.069)
            source: "meshes/btn_OPEN_TEST_MESH_mesh.mesh"
            materials: [
                grey_BUTTON_material
            ]
        }
        Model {
            id: btn_PAUSE_TEST
            objectName: "BTN_PAUSE_TEST"
            position: Qt.vector3d(-0.033, 0.0235, 0.069)
            source: "meshes/btn_PAUSE_TEST_MESH_mesh.mesh"
            materials: [
                grey_BUTTON_material
            ]
        }
        Model {
            id: btn_PLAY_TEST
            objectName: "BTN_PLAY_TEST"
            position: Qt.vector3d(-0.048, 0.0235, 0.069)
            source: "meshes/btn_PLAY_TEST_MESH_mesh.mesh"
            materials: [
                grey_BUTTON_material
            ]
        }
        Model {
            id: btn_STOP_TEST
            objectName: "BTN_STOP_TEST"
            position: Qt.vector3d(-0.018, 0.0235, 0.069)
            source: "meshes/btn_STOP_TEST_MESH_mesh.mesh"
            materials: [
                grey_BUTTON_material
            ]
        }
        Node {
            id: disc_ROOT
            objectName: "DISC_ROOT"
            position: Qt.vector3d(0, 0.014, 0)
            Model {
                id: disc_TEST
                objectName: "DISC_TEST"
                source: "meshes/disc_TEST_MESH_mesh.mesh"
                materials: [
                    grey_DISC_material
                ]
            }
        }
        Model {
            id: lcd_TEST
            objectName: "LCD_TEST"
            position: Qt.vector3d(0.033, 0.0221, 0.069)
            source: "meshes/lcd_TEST_MESH_mesh.mesh"
            materials: [
                grey_LCD_material
            ]
        }
        Node {
            id: lid_ROOT
            objectName: "LID_ROOT"
            position: Qt.vector3d(0, 0.028, -0.074)
            Model {
                id: lid_TEST
                objectName: "LID_TEST"
                source: "meshes/lid_TEST_MESH_mesh.mesh"
                materials: [
                    grey_LID_material
                ]
            }
        }
        Model {
            id: spindle_TEST
            objectName: "SPINDLE_TEST"
            source: "meshes/spindle_TEST_MESH_mesh.mesh"
            materials: [
                grey_BODY_material
            ]
        }
    }

    // Animations:
}
