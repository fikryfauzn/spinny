import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "PlayerUi"
    when: windowShown

    Component { id: appComponent; App.Main {} }

    function test_disc_lifecycle_without_3d_assets() {
        const app = createTemporaryObject(appComponent, null)
        verify(app !== null)
        const bridge = app.appliance
        compare(bridge.lidState, "Closed")
        compare(bridge.discState, "Absent")
        verify(!app.insertDisc())

        const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
        verify(app.selectDiscUrl(url))
        verify(bridge.requestOpen())
        tryCompare(bridge, "lidState", "Open", 2000)
        verify(app.insertDisc(), bridge.lastRejection)
        tryCompare(bridge, "discState", "Seated", 5000)

        verify(app.removeDisc(), bridge.lastRejection)
        compare(bridge.discState, "Absent")
        verify(bridge.requestClose())
        tryCompare(bridge, "lidState", "Closed", 2000)
    }
}
