import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "ButtonTravel"
    function init() { failOnWarning(/.*TypeError.*/) }
    Component { id: modelComponent; Model { position: Qt.vector3d(-0.033, 0.013, 0.073) } }
    Component { id: travelComponent; App.ButtonTravel {} }

    function test_press_release_data() {
        return [{tag: "top", offset: Qt.vector3d(0, -0.0006, 0)},
                {tag: "front", offset: Qt.vector3d(0, 0, -0.0006)}]
    }
    function test_press_release(data) {
        const model = createTemporaryObject(modelComponent, null)
        const travel = createTemporaryObject(travelComponent, null,
            {model: model, travelOffset: data.offset})
        verify(travel.ready)
        verify(travel.press())
        wait(80)
        fuzzyCompare(model.position.x, -0.033, 0.000001)
        fuzzyCompare(model.position.y, 0.013 + data.offset.y, 0.000001)
        fuzzyCompare(model.position.z, 0.073 + data.offset.z, 0.000001)
        compare(travel.pressed, true)
        travel.release()
        wait(120)
        fuzzyCompare(model.position.y, 0.013, 0.000001)
        fuzzyCompare(model.position.z, 0.073, 0.000001)
        compare(travel.pressed, false)
    }
    function test_rapid_repress_and_repeated_clicks_do_not_drift() {
        const model = createTemporaryObject(modelComponent, null)
        const travel = createTemporaryObject(travelComponent, null,
            {model: model, travelOffset: Qt.vector3d(0, -0.0006, 0)})
        for (let i = 0; i < 4; ++i) {
            verify(travel.press())
            wait(20)
            travel.release()
            wait(20)
        }
        verify(travel.press())
        wait(80)
        fuzzyCompare(model.position.y, 0.0124, 0.000001)
        travel.release()
        wait(120)
        fuzzyCompare(model.position.y, 0.013, 0.000001)
        fuzzyCompare(travel.restPosition.y, 0.013, 0.000001)
    }
    function test_replacing_handle_restores_old_and_captures_new_rest() {
        const first = createTemporaryObject(modelComponent, null)
        const second = createTemporaryObject(modelComponent, null,
            {position: Qt.vector3d(-0.033, 0.025, 0.073)})
        fuzzyCompare(second.y, 0.025, 0.000001)
        const travel = createTemporaryObject(travelComponent, null,
            {model: first, travelOffset: Qt.vector3d(0, -0.0006, 0)})
        verify(travel.press())
        wait(80)
        travel.model = second
        fuzzyCompare(first.y, 0.013, 0.000001)
        compare(travel.pressed, false)
        verify(travel.press())
        wait(80)
        fuzzyCompare(second.y, 0.0244, 0.000001)
        travel.model = null
        fuzzyCompare(second.y, 0.025, 0.000001)
        verify(!travel.ready)
        verify(!travel.press())
    }
    function test_destroyed_handle_is_not_ready() {
        const model = modelComponent.createObject(null)
        const travel = createTemporaryObject(travelComponent, null,
            {model: model, travelOffset: Qt.vector3d(0, -0.0006, 0)})
        verify(travel.press())
        model.destroy()
        wait(20)
        verify(!travel.ready)
        verify(!travel.press())
    }
}
