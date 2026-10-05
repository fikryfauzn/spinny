import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "TransportControls"
    Component {
        id: sceneComponent
        Node {
            objectName: "PLAYER_ROOT"
            Model { objectName: "BTN_PLAY_TEST"; y: 0.0235 }
            Model { objectName: "BTN_PAUSE_TEST"; y: 0.0235 }
            Model { objectName: "BTN_STOP_TEST"; y: 0.0235 }
            Model { objectName: "BTN_PREVIOUS_TEST"; z: 0.073 }
            Model { objectName: "BTN_NEXT_TEST"; z: 0.073 }
            Model { objectName: "BTN_OPEN_TEST"; y: 0.0235 }
            Model { objectName: "DISC_TEST" }
        }
    }
    Component {
        id: recorderComponent
        QtObject {
            property string lidState: "Closed"
            property var calls: []
            property bool accepted: true
            function record(name) { calls = calls.concat([name]); return accepted }
            function requestPlay() { return record("PLAY") }
            function requestPause() { return record("PAUSE") }
            function requestStop() { return record("STOP") }
            function requestPrevious() { return record("PREVIOUS") }
            function requestNext() { return record("NEXT") }
            function requestOpen() { return record("OPEN") }
            function requestClose() { return record("CLOSE") }
        }
    }
    Component { id: surfaceComponent; App.ControlSurface {} }
    Component { id: extraModel; Model {} }
    Component { id: impostor; Node { objectName: "BTN_PLAY_TEST" } }

    function setup() {
        const root = createTemporaryObject(sceneComponent, null)
        const bridge = createTemporaryObject(recorderComponent, null)
        const surface = createTemporaryObject(surfaceComponent, null,
            {sceneRoot: root, appliance: bridge, view3d: null})
        verify(surface.assetReady)
        return {root: root, bridge: bridge, surface: surface}
    }
    function test_dispatch_once_on_release_data() {
        return ["PLAY", "PAUSE", "STOP", "PREVIOUS", "NEXT", "OPEN"].map(
            function(name) { return {tag: name, name: name} })
    }
    function test_dispatch_once_on_release(data) {
        const s = setup()
        const button = s.surface.buttonForName("BTN_" + data.name + "_TEST")
        verify(s.surface.beginPress(button))
        wait(100)
        compare(s.bridge.calls.length, 0)
        verify(s.surface.finishPress(button))
        compare(s.bridge.calls.join(","), data.name)
        verify(!s.surface.finishPress(button))
        compare(s.bridge.calls.length, 1)
    }
    function test_open_uses_authoritative_lid_projection_data() {
        return [{tag: "open", state: "Open", command: "CLOSE"},
                {tag: "opening", state: "Opening", command: "OPEN"},
                {tag: "closing", state: "Closing", command: "OPEN"}]
    }
    function test_open_uses_authoritative_lid_projection(data) {
        const s = setup()
        s.bridge.lidState = data.state
        const button = s.surface.buttonForName("BTN_OPEN_TEST")
        verify(s.surface.beginPress(button))
        verify(s.surface.finishPress(button))
        compare(s.bridge.calls.join(","), data.command)
    }
    function test_cancel_outside_other_button_and_long_hold() {
        const s = setup()
        const play = s.surface.buttonForName("BTN_PLAY_TEST")
        const stop = s.surface.buttonForName("BTN_STOP_TEST")
        verify(s.surface.beginPress(play))
        wait(350)
        compare(s.bridge.calls.length, 0)
        verify(!s.surface.beginPress(stop))
        verify(!s.surface.finishPress(stop))
        verify(s.surface.beginPress(play))
        verify(!s.surface.finishPress(null))
        verify(s.surface.beginPress(play))
        s.surface.cancelPress()
        verify(!s.surface.finishPress(play))
        compare(s.bridge.calls.length, 0)
        wait(120)
        fuzzyCompare(play.y, 0.0235, 0.000001)
        verify(!s.surface.beginPress(s.root.children[6]))
    }
    function test_rejection_still_has_travel() {
        const s = setup()
        s.bridge.accepted = false
        const button = s.surface.buttonForName("BTN_NEXT_TEST")
        verify(s.surface.beginPress(button))
        wait(80)
        fuzzyCompare(button.z, 0.0724, 0.000001)
        verify(!s.surface.finishPress(button))
        compare(s.bridge.calls.join(","), "NEXT")
        wait(120)
        fuzzyCompare(button.z, 0.073, 0.000001)
    }
    function test_contract_loss_cancels_capture() {
        const s = setup()
        const button = s.surface.buttonForName("BTN_PLAY_TEST")
        verify(s.surface.beginPress(button))
        wait(80)
        button.objectName = "INVALID"
        tryCompare(s.surface, "assetReady", false)
        compare(s.surface.pressedName, "")
        verify(!s.surface.finishPress(button))
        compare(s.bridge.calls.length, 0)
        wait(120)
        fuzzyCompare(button.y, 0.0235, 0.000001)
    }
    function test_valid_scene_replacement_cancels_original_mesh_capture() {
        const s = setup()
        const original = s.surface.buttonForName("BTN_PLAY_TEST")
        verify(s.surface.beginPress(original))
        wait(80)
        const replacement = createTemporaryObject(sceneComponent, null)
        s.surface.sceneRoot = replacement
        verify(s.surface.assetReady)
        const replacementButton = s.surface.buttonForName("BTN_PLAY_TEST")
        verify(replacementButton !== original)
        compare(s.surface.pressedName, "")
        verify(!s.surface.finishPress(replacementButton))
        compare(s.bridge.calls.length, 0)
        wait(120)
        fuzzyCompare(original.y, 0.0235, 0.000001)
        fuzzyCompare(replacementButton.y, 0.0235, 0.000001)
    }
    function test_duplicate_wrong_parent_and_non_model_contracts() {
        const s = setup()
        const duplicate = createTemporaryObject(extraModel, s.root, {objectName: "BTN_PLAY_TEST"})
        tryCompare(s.surface, "assetReady", false)
        duplicate.objectName = "OTHER"
        tryCompare(s.surface, "assetReady", true)
        const button = s.surface.buttonForName("BTN_PLAY_TEST")
        button.parent = s.root.children[6]
        tryCompare(s.surface, "assetReady", false)
        button.parent = s.root
        tryCompare(s.surface, "assetReady", true)
        button.objectName = "NOT_PLAY"
        createTemporaryObject(impostor, s.root)
        tryCompare(s.surface, "assetReady", false)
        verify(!s.surface.beginPress(button))
    }
}
