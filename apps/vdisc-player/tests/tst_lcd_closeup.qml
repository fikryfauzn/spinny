import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "LcdCloseup"
    width: 500; height: 400
    visible: true
    when: windowShown
    QtObject { id: authored; property real x: 20; property real fov: 60 }
    Component { id: cameraComponent; PerspectiveCamera {
        position: Qt.vector3d(authored.x,20,36)
        eulerRotation: Qt.vector3d(-25,29,0)
        fieldOfView: authored.fov
    } }
    Component { id: sceneComponent; Node {
        objectName: "PLAYER_ROOT"
        scale: Qt.vector3d(100,100,100)
        property alias screen: screen
        property alias disc: disc
        Model { id: screen; objectName: "LCD_TEST"; position: Qt.vector3d(0.033,0.0221,0.069) }
        Node { objectName: "LID_ROOT"; Model { objectName: "LID_TEST" } }
        Node { objectName: "DISC_ROOT"; Model { id: disc; objectName: "DISC_TEST" } }
    } }
    Component { id: inspectionComponent; App.DiscInspection {} }
    function setup() {
        failOnWarning(/.*/)
        authored.x = 20; authored.fov = 60
        const scene = createTemporaryObject(sceneComponent,null)
        const camera = createTemporaryObject(cameraComponent,null)
        const inspection = createTemporaryObject(inspectionComponent,this,{width:500,height:400,
            sceneRoot:scene,discModel:scene.disc,camera:camera,windowActive:true,filePickerVisible:false})
        verify(inspection !== null)
        return {scene:scene,camera:camera,inspection:inspection}
    }
    function test_targets_screen_then_restores_live_bindings() {
        const f = setup()
        verify(f.inspection.toggleLcdCloseup())
        fuzzyCompare(f.camera.x,3.3,0.00001)
        fuzzyCompare(f.camera.y,10.225,0.00001)
        fuzzyCompare(f.camera.z,12.9,0.00001)
        fuzzyCompare(f.camera.eulerRotation.x,-53.130102,0.00001)
        compare(f.camera.eulerRotation.y,0); compare(f.camera.fieldOfView,40)
        authored.x = 25; authored.fov = 55
        verify(f.inspection.toggleLcdCloseup())
        compare(f.camera.position,Qt.vector3d(25,20,36))
        compare(f.camera.eulerRotation,Qt.vector3d(-25,29,0))
        compare(f.camera.fieldOfView,55)
        authored.x = 22; compare(f.camera.x,22)
    }
    function test_guards_actual_mouse_and_shortcut() {
        const f = setup()
        f.inspection.filePickerVisible = true
        tryCompare(f.inspection.closeupButton,"x",314)
        verify(!f.inspection.toggleLcdCloseup())
        verify(!f.inspection.lcdShortcut.enabled)
        mouseClick(f.inspection.closeupButton,30,15)
        compare(f.camera.position,Qt.vector3d(20,20,36))
        f.inspection.filePickerVisible = false
        verify(f.inspection.closeupButton.visible, "Button effective visibility")
        mouseClick(f.inspection.closeupButton,30,15)
        verify(f.inspection.lcdCloseupEnabled)
        keyClick(Qt.Key_L)
        tryCompare(f.inspection,"lcdCloseupEnabled",false)
        f.inspection.windowActive = false
        verify(!f.inspection.lcdShortcut.enabled)
        verify(!f.inspection.toggleLcdCloseup())
    }
    function test_asset_loss_and_teardown_restore_camera() {
        const f = setup()
        verify(f.inspection.toggleLcdCloseup())
        f.scene.screen.objectName = "MISSING"
        compare(f.camera.position,Qt.vector3d(20,20,36))
        verify(!f.inspection.lcdCloseupEnabled)
        verify(!f.inspection.toggleLcdCloseup())
        f.scene.screen.objectName = "LCD_TEST"
        verify(f.inspection.toggleLcdCloseup())
        f.inspection.destroy(); wait(0)
        compare(f.camera.position,Qt.vector3d(20,20,36))
        compare(f.camera.fieldOfView,60)
    }
}
