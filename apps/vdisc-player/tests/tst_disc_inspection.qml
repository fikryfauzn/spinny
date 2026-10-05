import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "DiscInspection"
    width: 400
    height: 300
    when: windowShown
    Component {
        id: sceneComponent
        Node {
            id: scene
            objectName: "PLAYER_ROOT"
            property bool authoredVisible: true
            property alias lid: lid
            property alias hinge: hinge
            property alias disc: disc
            property alias deck: deck
            property alias body: body
            Model { id: body; objectName: "BODY_TEST" }
            Node {
                id: hinge
                objectName: "LID_ROOT"
                eulerRotation.x: -30
                Model { id: lid; objectName: "LID_TEST"; visible: scene.authoredVisible }
            }
            Node {
                id: deck
                objectName: "DISC_ROOT"
                Model { id: disc; objectName: "DISC_TEST" }
            }
        }
    }
    Component { id: inspectionComponent; App.DiscInspection {} }
    Component { id: extraLid; Model { objectName: "LID_TEST" } }
    Component { id: wrongLid; Node { objectName: "LID_TEST" } }
    function setup() {
        failOnWarning(/.*/)
        const scene = createTemporaryObject(sceneComponent, null)
        const inspection = createTemporaryObject(inspectionComponent, this, {
            width: 400, height: 300, sceneRoot: scene, discModel: scene.disc,
            windowActive: true, filePickerVisible: false})
        verify(inspection !== null)
        verify(inspection.assetReady)
        return {scene: scene, inspection: inspection}
    }
    function test_marker_annulus_and_following() {
        const f = setup()
        const marker = f.inspection.marker
        compare(marker.parent, f.scene.disc)
        compare(marker.objectName, "")
        verify(!marker.pickable)
        fuzzyCompare(marker.x, 0.0325, 0.000001)
        fuzzyCompare(marker.y, 0.0007, 0.000001)
        fuzzyCompare(marker.z, 0, 0.000001)
        fuzzyCompare(marker.scale.x * 100, 0.040, 0.000001)
        fuzzyCompare(marker.scale.y * 100, 0.0001, 0.000001)
        fuzzyCompare(marker.scale.z * 100, 0.0015, 0.000001)
        fuzzyCompare(marker.x - marker.scale.x * 50, 0.0125, 0.000001)
        fuzzyCompare(marker.x + marker.scale.x * 50, 0.0525, 0.000001)
        compare(marker.materials[0].lighting, DefaultMaterial.NoLighting)
        f.scene.deck.x = 0.1
        f.scene.disc.eulerRotation.y = 90
        const position = marker.mapPositionToScene(Qt.vector3d(0, 0, 0))
        fuzzyCompare(position.x, 0.1, 0.000001)
        fuzzyCompare(position.z, -0.0325, 0.000001)
        f.scene.deck.visible = false
        verify(!marker.sceneTransform.toString().includes("NaN"))
        compare(marker.parent.parent.visible, false)
        f.inspection.discModel = null
        verify(!marker.visible)
    }
    function test_cutaway_only_hides_lid_and_restores_binding() {
        const f = setup()
        verify(!f.inspection.cutawayEnabled)
        verify(f.scene.lid.visible)
        verify(f.inspection.toggleCutaway())
        verify(!f.scene.lid.visible)
        verify(f.scene.body.visible)
        compare(f.scene.hinge.eulerRotation.x, -30)
        f.scene.authoredVisible = false
        verify(f.inspection.toggleCutaway())
        verify(!f.scene.lid.visible)
        f.scene.authoredVisible = true
        verify(f.scene.lid.visible)
        for (let i = 0; i < 3; i++) {
            verify(f.inspection.toggleCutaway())
            verify(!f.scene.lid.visible)
            verify(f.inspection.toggleCutaway())
            verify(f.scene.lid.visible)
        }
    }
    function test_original_false_and_teardown() {
        const f = setup()
        f.scene.lid.visible = false
        verify(f.inspection.toggleCutaway())
        verify(f.inspection.toggleCutaway())
        verify(!f.scene.lid.visible)
        f.scene.lid.visible = true
        verify(f.inspection.toggleCutaway())
        f.inspection.destroy()
        wait(30)
        verify(f.scene.lid.visible)
    }
    function test_replacement_and_asset_loss_restore_outgoing() {
        const f = setup()
        verify(f.inspection.toggleCutaway())
        const scene2 = createTemporaryObject(sceneComponent, null)
        f.inspection.sceneRoot = scene2
        verify(f.scene.lid.visible)
        verify(!scene2.lid.visible)
        scene2.lid.objectName = "MISSING"
        verify(!f.inspection.assetReady)
        verify(scene2.lid.visible)
        verify(!f.inspection.toggleCutaway())
    }
    function test_malformed_data() {
        return [{tag: "missing"}, {tag: "duplicate"}, {tag: "wrongtype"},
                {tag: "parent"}, {tag: "missingroot"}]
    }
    function test_malformed(data) {
        const f = setup()
        if (data.tag === "duplicate")
            createTemporaryObject(extraLid, f.scene.hinge)
        else if (data.tag === "wrongtype") {
            f.scene.lid.objectName = "MISSING"
            createTemporaryObject(wrongLid, f.scene.hinge)
        } else if (data.tag === "parent")
            f.scene.lid.parent = f.scene
        else if (data.tag === "missingroot")
            f.scene.hinge.objectName = "MISSING"
        else
            f.scene.lid.objectName = "MISSING"
        verify(!f.inspection.assetReady)
        verify(f.inspection.assetError.length > 0)
        verify(!f.inspection.toggleCutaway())
    }
    function test_destroyed_lid_is_safe() {
        const f = setup()
        verify(f.inspection.toggleCutaway())
        f.scene.lid.destroy()
        wait(30)
        verify(!f.inspection.assetReady)
        verify(!f.inspection.toggleCutaway())
    }
}
