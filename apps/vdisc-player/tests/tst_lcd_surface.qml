import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "LcdSurface"
    when: windowShown
    QtObject {
        id: a
        property int lcdTrackNumber: 1
        property int lcdElapsedMs: 0
        property int lcdTotalTracks: 2
        property int lcdTotalMs: 4000
        property string lcdPlaybackStatus: "Stopped"
        property string lcdPlayMode: "Normal"
        property bool lcdHold: false
        property bool lcdAvls: false
        property string lcdMessage: ""
    }
    Component {
        id: sceneComponent
        Node {
            scale: Qt.vector3d(100,100,100)
            property alias player: player
            property alias screen: screen
            Node {
                id: player; objectName: "PLAYER_ROOT"
                Model {
                    id: screen; objectName: "LCD_TEST"
                    position: Qt.vector3d(0.033,0.0221,0.069)
                    source: "#Cube"
                }
            }
        }
    }
    Component { id: surfaceComponent; App.LcdSurface {} }
    Component { id: modelComponent; Model { source: "#Cube" } }
    Component { id: nodeComponent; Node {} }
    function scene() { return createTemporaryObject(sceneComponent, null) }
    function surface(root) { return createTemporaryObject(surfaceComponent,this,{appliance:a,sceneRoot:root}) }
    function test_alignment_and_reading_basis() {
        const root = scene(); const s = surface(root)
        verify(s.assetReady); compare(s.lcdModel,root.screen)
        compare(s.plane.parent,root.screen); compare(s.plane.pickable,false)
        const left = s.plane.mapPositionToScene(Qt.vector3d(-50,0,0))
        const right = s.plane.mapPositionToScene(Qt.vector3d(50,0,0))
        const top = s.plane.mapPositionToScene(Qt.vector3d(0,50,0))
        const bottom = s.plane.mapPositionToScene(Qt.vector3d(0,-50,0))
        fuzzyCompare(right.x-left.x,4.5,0.00001)
        fuzzyCompare(bottom.z-top.z,0.6,0.00001)
        fuzzyCompare(s.plane.scenePosition.y-root.screen.scenePosition.y,0.015,0.00001)
        const normal = s.plane.mapDirectionToScene(Qt.vector3d(0,0,1))
        verify(normal.y > 0); fuzzyCompare(normal.x,0,0.00001); fuzzyCompare(normal.z,0,0.00001)
        compare(s.display.width,900); compare(s.display.height,120)
        compare(s.plane.materials[0].baseColorMap.sourceItem,s.display)
        compare(s.plane.materials[0].lighting,PrincipledMaterial.NoLighting)
    }
    function test_invalid_contracts_fail_closed() {
        const root = scene(); const s = surface(root)
        root.screen.objectName = "MISSING"
        tryCompare(s,"assetReady",false); verify(!s.plane.visible)
        root.screen.objectName = "LCD_TEST"; tryCompare(s,"assetReady",true)
        const duplicate = createTemporaryObject(modelComponent,root.player,{objectName:"LCD_TEST"})
        tryCompare(s,"assetReady",false); verify(s.assetError.length > 0)
        duplicate.objectName = "IGNORED"; tryCompare(s,"assetReady",true)
        root.screen.parent = root
        tryCompare(s,"assetReady",false)
        root.screen.parent = root.player; tryCompare(s,"assetReady",true)
        root.screen.objectName = "IGNORED"
        createTemporaryObject(nodeComponent,root.player,{objectName:"LCD_TEST"})
        tryCompare(s,"assetReady",false)
        compare(a.lcdPlaybackStatus,"Stopped")
    }
    function test_replacement_destruction_and_runtime_children() {
        failOnWarning(/.*Binding loop.*/)
        const first = scene(); const s = surface(first)
        verify(s.assetReady)
        createTemporaryObject(modelComponent,first.screen,{objectName:"LCD_TEST"})
        verify(s.assetReady)
        const second = scene()
        s.sceneRoot = second
        tryCompare(s,"lcdModel",second.screen)
        compare(s.plane.parent,second.screen)
        first.destroy(); wait(0); verify(s.assetReady)
        s.sceneRoot = null
        tryCompare(s,"assetReady",false); compare(s.plane.parent,null); verify(!s.plane.visible)
        s.sceneRoot = second; tryCompare(s,"assetReady",true)
        second.destroy(); wait(0)
        tryCompare(s,"assetReady",false); verify(!s.plane.visible)
    }
}
