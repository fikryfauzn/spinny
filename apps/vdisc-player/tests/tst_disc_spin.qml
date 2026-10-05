import QtQuick
import QtTest
import QtQuick3D
import "../qml" as App
import "../qml/DiscSpinPolicy.js" as Policy

TestCase {
    name: "DiscSpin"
    width: 200
    height: 200
    when: windowShown
    Component {
        id: sceneComponent
        Node {
            objectName: "PLAYER_ROOT"
            property alias disc: disc
            property alias root: discRoot
            Node {
                id: discRoot
                objectName: "DISC_ROOT"
                Model { id: disc; objectName: "DISC_TEST"; eulerRotation.y: 17 }
            }
        }
    }
    Component {
        id: stateComponent
        QtObject {
            property string transportState: "Stopped"
            property string lidState: "Closed"
            property string discState: "Seated"
        }
    }
    Component { id: spinComponent; App.DiscSpin {} }
    Component { id: extraModel; Model { objectName: "DISC_TEST" } }
    Component { id: wrongNode; Node { objectName: "DISC_TEST" } }
    function setup() {
        failOnWarning(/.*/)
        const scene = createTemporaryObject(sceneComponent, null)
        const state = createTemporaryObject(stateComponent, null)
        const spin = createTemporaryObject(spinComponent, this, {sceneRoot: scene, appliance: state})
        verify(spin !== null)
        verify(spin.assetReady)
        compare(scene.disc.eulerRotation.y, 17)
        return {scene: scene, state: state, spin: spin}
    }
    function play(f) {
        f.state.transportState = "Playing"
        tryCompare(f.spin, "speed", 180, 650)
    }
    function test_ramp_and_idle() {
        const f = setup()
        compare(f.spin.speed, 0)
        verify(!f.spin.frameRunning)
        f.state.transportState = "Playing"
        wait(140)
        verify(f.spin.speed > 0 && f.spin.speed < 180)
        tryCompare(f.spin, "speed", 180, 450)
        verify(f.spin.frameCount > 0)
        verify(f.scene.disc.eulerRotation.y !== 17)
        f.state.transportState = "Paused"
        wait(120)
        verify(f.spin.speed > 0 && f.spin.speed < 180)
        tryCompare(f.spin, "speed", 0, 450)
        verify(!f.spin.frameRunning)
        const angle = f.spin.angle
        wait(100)
        compare(f.spin.angle, angle)
        compare(f.scene.disc.eulerRotation.x, 0)
        compare(f.scene.disc.eulerRotation.z, 0)
    }
    function test_same_target_does_not_restart_and_reversal_has_no_jump() {
        const f = setup()
        f.state.transportState = "Playing"
        wait(250)
        const speed = f.spin.speed
        const angle = f.spin.angle
        f.state.transportState = "SeekingForward"
        compare(f.spin.speed, speed)
        compare(f.spin.angle, angle)
        wait(200)
        compare(f.spin.speed, 180)
        f.state.transportState = "Paused"
        wait(120)
        const slowing = f.spin.speed
        const pausedAngle = f.spin.angle
        f.state.transportState = "Playing"
        compare(f.spin.speed, slowing)
        compare(f.spin.angle, pausedAngle)
        tryCompare(f.spin, "speed", 180, 650)
        f.state.transportState = "SeekingBackward"
        compare(f.spin.speed, 180)
        verify(!f.spin.rampRunning)
    }
    function test_immediate_stops_data() {
        return [{tag: "lid", field: "lidState", value: "Opening"},
                {tag: "disc", field: "discState", value: "Removing"},
                {tag: "unknown", field: "transportState", value: "Unknown"}]
    }
    function test_immediate_stops(data) {
        const f = setup()
        play(f)
        const angle = f.spin.angle
        f.state[data.field] = data.value
        compare(f.spin.speed, 0)
        verify(!f.spin.rampRunning)
        compare(f.spin.angle, angle)
        wait(60)
        compare(f.spin.angle, angle)
    }
    function test_absent_resets_and_replacement_does_not_inherit_motion() {
        const f = setup()
        play(f)
        f.state.discState = "Absent"
        compare(f.spin.angle, 0)
        compare(f.scene.disc.eulerRotation.y, 17)
        f.state.discState = "Seated"
        tryCompare(f.spin, "speed", 180, 650)
        const replacement = createTemporaryObject(sceneComponent, null)
        replacement.disc.eulerRotation.y = 29
        f.spin.sceneRoot = replacement
        compare(f.scene.disc.eulerRotation.y, 17)
        compare(replacement.disc.eulerRotation.y, 29)
        compare(f.spin.speed, 0)
        wait(100)
        compare(f.spin.speed, 0)
        compare(replacement.disc.eulerRotation.y, 29)
    }
    function test_malformed_hierarchy_data() {
        return [{tag: "missing"}, {tag: "duplicate"}, {tag: "wrongtype"}, {tag: "parent"}]
    }
    function test_malformed_hierarchy(data) {
        const f = setup()
        play(f)
        if (data.tag === "missing")
            f.scene.disc.objectName = "MISSING"
        else if (data.tag === "duplicate")
            createTemporaryObject(extraModel, f.scene.root)
        else if (data.tag === "wrongtype") {
            f.scene.disc.objectName = "MISSING"
            createTemporaryObject(wrongNode, f.scene.root)
        } else
            f.scene.disc.parent = f.scene
        verify(!f.spin.assetReady)
        verify(f.spin.assetError.length > 0)
        compare(f.spin.speed, 0)
        compare(f.scene.disc.eulerRotation.y, 17)
    }
    function test_destroyed_handle_and_teardown_are_safe() {
        const f = setup()
        play(f)
        f.scene.disc.destroy()
        wait(30)
        verify(!f.spin.assetReady)
        compare(f.spin.speed, 0)
        const g = setup()
        play(g)
        g.spin.destroy()
        wait(30)
        compare(g.scene.disc.eulerRotation.y, 17)
    }
    function test_targets_data() {
        return [
            {tag: "playing", transport: "Playing", lid: "Closed", disc: "Seated", ready: true, want: 180},
            {tag: "forward", transport: "SeekingForward", lid: "Closed", disc: "Seated", ready: true, want: 180},
            {tag: "backward", transport: "SeekingBackward", lid: "Closed", disc: "Seated", ready: true, want: 180},
            {tag: "paused", transport: "Paused", lid: "Closed", disc: "Seated", ready: true, want: 0},
            {tag: "stopped", transport: "Stopped", lid: "Closed", disc: "Seated", ready: true, want: 0},
            {tag: "unknown", transport: "Unknown", lid: "Closed", disc: "Seated", ready: true, want: null},
            {tag: "invalid", transport: "Playing", lid: "Closed", disc: "Seated", ready: false, want: null},
            {tag: "opening", transport: "Playing", lid: "Opening", disc: "Seated", ready: true, want: null},
            {tag: "open", transport: "Playing", lid: "Open", disc: "Seated", ready: true, want: null},
            {tag: "closing", transport: "Playing", lid: "Closing", disc: "Seated", ready: true, want: null},
            {tag: "absent", transport: "Playing", lid: "Closed", disc: "Absent", ready: true, want: null},
            {tag: "inserting", transport: "Playing", lid: "Closed", disc: "Inserting", ready: true, want: null},
            {tag: "removing", transport: "Playing", lid: "Closed", disc: "Removing", ready: true, want: null}
        ]
    }
    function test_targets(data) {
        compare(Policy.targetSpeed(data.transport, data.lid, data.disc, data.ready), data.want)
    }
    function test_elapsed_data() {
        return [
            {tag: "10ms", angle: 10, speed: 180, dt: 0.01, want: 11.8},
            {tag: "20ms", angle: 10, speed: 180, dt: 0.02, want: 13.6},
            {tag: "stall", angle: 10, speed: 180, dt: 2, want: 19},
            {tag: "wrap", angle: 359, speed: 180, dt: 0.02, want: 2.6},
            {tag: "idle", angle: 10, speed: 0, dt: 0.02, want: 10},
            {tag: "zero", angle: 10, speed: 180, dt: 0, want: 10},
            {tag: "negative", angle: 10, speed: 180, dt: -1, want: 10},
            {tag: "nan", angle: 10, speed: 180, dt: NaN, want: 10},
            {tag: "infinite", angle: 10, speed: 180, dt: Infinity, want: 10}
        ]
    }
    function test_elapsed(data) {
        fuzzyCompare(Policy.advanceAngle(data.angle, data.speed, data.dt), data.want, 0.000001)
    }
}
