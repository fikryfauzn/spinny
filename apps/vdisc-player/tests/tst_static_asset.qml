import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "StaticGreybox"

    Component {
        id: appComponent
        App.Main {}
    }

    function collect(node, names) {
        if (node.objectName && node.position !== undefined)
            names[node.objectName] = node
        for (let child of node.children)
            collect(child, names)
    }

    function test_greybox_is_addressable_at_centimeter_scale() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const greybox = app.greybox
            verify(greybox !== undefined && greybox !== null)
            compare(greybox.scale.x, 100)
            compare(greybox.scale.y, 100)
            compare(greybox.scale.z, 100)
            compare(greybox.eulerRotation.x, 0)
            compare(greybox.eulerRotation.y, 0)
            compare(greybox.eulerRotation.z, 0)

            const names = {}
            collect(greybox, names)
            const expected = ["PLAYER_ROOT", "BODY_TEST", "LID_ROOT", "LID_TEST",
                              "DISC_ROOT", "DISC_TEST", "SPINDLE_TEST", "BTN_PLAY_TEST",
                              "BTN_PAUSE_TEST", "BTN_STOP_TEST", "BTN_OPEN_TEST", "LCD_TEST"]
            compare(Object.keys(names).sort().join(","), expected.sort().join(","))
            compare(names.LID_ROOT.parent, names.PLAYER_ROOT)
            compare(names.LID_TEST.parent, names.LID_ROOT)
            compare(names.DISC_ROOT.parent, names.PLAYER_ROOT)
            compare(names.DISC_TEST.parent, names.DISC_ROOT)
            fuzzyCompare(names.LID_ROOT.scenePosition.y, 2.8, 0.01)
            fuzzyCompare(names.LID_ROOT.scenePosition.z, -7.4, 0.01)
            fuzzyCompare(names.DISC_ROOT.scenePosition.y, 1.4, 0.01)
        } finally {
            app.destroy()
        }
    }
}
