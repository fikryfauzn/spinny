import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "RailControl"
    width: 800; height: 600
    Component {
        id: fixture
        Item {
            width: 800; height: 600
            property alias view: view
            property alias root: root
            property alias rail: rail
            property alias knob: knob
            View3D {
                id: view; anchors.fill: parent
                camera: PerspectiveCamera { position: Qt.vector3d(0, 2, 30); clipNear: 0.1; clipFar: 200 }
                Node {
                    id: root; objectName: "PLAYER_ROOT"; scale: Qt.vector3d(100, 100, 100)
                    Model { id: rail; objectName: "VOLUME_RAIL_TEST"; position: Qt.vector3d(0.043, 0.013, 0.073) }
                    Model { id: knob; objectName: "VOLUME_KNOB_TEST"; position: Qt.vector3d(0.043, 0.013, 0.0735) }
                }
            }
        }
    }
    Component { id: bridgeComponent; App.Main {} }
    Component { id: controlComponent; App.RailControl {} }
    function setup(kind) {
        const scene = createTemporaryObject(fixture, this)
        const app = createTemporaryObject(bridgeComponent, null)
        tryCompare(app.contentItem, "width", app.width, 500)
        if (kind === "Hold") {
            scene.rail.objectName = "HOLD_RAIL_TEST"
            scene.knob.objectName = "HOLD_KNOB_TEST"
        }
        const control = createTemporaryObject(controlComponent, this,
            {appliance: app.appliance, view3d: scene.view, railModel: scene.rail, knobModel: scene.knob, kind: kind})
        verify(control.assetReady)
        return {scene: scene, app: app, control: control}
    }
    function screen(s, x) {
        return s.scene.view.mapFrom3DScene(s.scene.root.mapPositionToScene(Qt.vector3d(x, 0.013, 0.0735)))
    }
    function test_invalid_rays_have_no_mutation() {
        const s = setup("Volume")
        const rays = [[Qt.vector3d(0,0,1), Qt.vector3d(1,0,1)],
                      [Qt.vector3d(0,0,1), Qt.vector3d(0,0,2)],
                      [Qt.vector3d(NaN,0,1), Qt.vector3d(0,0,0)]]
        for (const ray of rays)
            compare(s.control.intersectRay(ray[0], ray[1]), null)
        compare(s.app.appliance.volume, 0.5)
    }
    function test_mapping_is_parent_local_data() {
        return [{tag:"default", rotation:Qt.vector3d(0,0,0), scale:Qt.vector3d(100,100,100), closeup:false},
                {tag:"transformed", rotation:Qt.vector3d(12,24,5), scale:Qt.vector3d(110,80,95), closeup:false},
                {tag:"closeup", rotation:Qt.vector3d(0,0,0), scale:Qt.vector3d(100,100,100), closeup:true}]
    }
    function test_mapping_is_parent_local(data) {
        const s = setup("Volume")
        s.scene.root.eulerRotation = data.rotation
        s.scene.root.scale = data.scale
        if (data.closeup) {
            s.scene.view.camera.position = Qt.vector3d(3.3, 10.21, 12.9)
            s.scene.view.camera.eulerRotation = Qt.vector3d(-53.130102, 0, 0)
            s.scene.view.camera.fieldOfView = 40
        }
        for (const x of [0.032, 0.043, 0.054]) {
            const point = screen(s, x)
            const mapped = s.control.pointOnPlane(point.x, point.y)
            verify(mapped !== null)
            fuzzyCompare(mapped.x, x, 0.000001)
        }
    }
    function test_volume_clamp_offset_and_cancel() {
        const s = setup("Volume")
        const grab = screen(s, 0.0435)
        verify(s.control.begin(grab.x, grab.y, true))
        compare(s.app.appliance.volume, 0.5)
        verify(!s.control.begin(grab.x, grab.y, true))
        const end = screen(s, 0.08)
        verify(s.control.update(end.x, end.y))
        compare(s.app.appliance.volume, 1)
        fuzzyCompare(s.scene.knob.x, 0.054, 0.000001)
        s.control.cancel()
        compare(s.app.appliance.volume, 1)
        const start = screen(s, 0.01)
        verify(s.control.begin(start.x, start.y, false))
        compare(s.app.appliance.volume, 0)
        fuzzyCompare(s.scene.knob.x, 0.032, 0.000001)
    }
    function test_hold_click_drag_midpoint_and_cancel() {
        const s = setup("Hold")
        const start = screen(s, 0.017)
        verify(s.control.begin(start.x, start.y, true))
        verify(s.control.finish(start.x, start.y))
        verify(s.app.appliance.holdEnabled)
        const end = screen(s, 0.023)
        verify(s.control.begin(end.x, end.y, false))
        verify(s.control.finish(end.x, end.y))
        verify(!s.app.appliance.holdEnabled)
        verify(s.control.begin(start.x, start.y, true))
        const middle = screen(s, 0.020)
        verify(s.control.update(middle.x, middle.y))
        verify(s.control.finish(middle.x, middle.y))
        verify(s.app.appliance.holdEnabled)
        verify(s.control.begin(end.x, end.y, true))
        verify(s.control.update(start.x, start.y))
        s.control.cancel()
        verify(s.app.appliance.holdEnabled)
        fuzzyCompare(s.scene.knob.x, 0.023, 0.000001)
    }
    function test_avls_repeated_clamp_and_hold_permit_volume() {
        const s = setup("Volume")
        verify(s.app.appliance.requestToggleAvls())
        verify(s.app.appliance.setHold(true))
        const high = screen(s, 0.06)
        for (let i = 0; i < 3; ++i) {
            verify(s.control.begin(high.x, high.y, false))
            verify(s.control.finish(high.x, high.y))
            fuzzyCompare(s.app.appliance.volume, 0.75, 0.00001)
            fuzzyCompare(s.scene.knob.x, 0.0485, 0.000001)
        }
        verify(s.app.appliance.setHold(false))
        verify(s.app.appliance.requestToggleAvls())
        fuzzyCompare(s.scene.knob.x, 0.0485, 0.000001)
    }
    function test_surface_has_one_owner_and_cancels_on_camera_change() {
        const s = setup("Volume")
        const surface = s.app.controls
        surface.windowActive = true
        const button = surface.buttonForName("BTN_PLAY_TEST")
        verify(surface.beginPress(button))
        verify(!surface.beginRail(surface.resolve("VOLUME_RAIL_TEST"), 10, 10))
        surface.cancelPress()
        const knob = surface.resolve("VOLUME_KNOB_TEST")
        const point = surface.view3d.mapFrom3DScene(knob.scenePosition)
        verify(surface.beginRail(knob, point.x, point.y))
        verify(!surface.beginPress(button))
        compare(surface.captureKind, "Volume")
        surface.inspectionRevision++
        compare(surface.captureKind, "")
        verify(!surface.volumeRail.active)
        compare(s.app.appliance.transportState, "Stopped")
    }
    function test_contract_loss_cancels_hold_preview() {
        const s = setup("Hold")
        const start = screen(s, 0.017)
        const end = screen(s, 0.023)
        verify(s.control.begin(start.x, start.y, true))
        verify(s.control.update(end.x, end.y))
        s.scene.rail.objectName = "INVALID"
        verify(!s.control.active)
        verify(!s.app.appliance.holdEnabled)
        verify(!s.control.finish(end.x, end.y))
    }
}
