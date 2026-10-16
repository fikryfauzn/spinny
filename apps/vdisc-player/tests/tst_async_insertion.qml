import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "AsyncInsertion"
    Component { id: appComponent; App.Main {} }

    function fixturePath() {
        const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
        return decodeURIComponent(url.substring("file://".length))
    }

    function test_valid_disc_waits_for_both_validation_and_motion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const a = app.appliance
            verify(a.requestOpen())
            tryCompare(a, "lidState", "Open", 1200)
            verify(a.requestInsert(fixturePath()))
            compare(a.discState, "Inserting")
            verify(a.insertionPending)
            verify(!a.requestClose())
            verify(!a.requestInsert(fixturePath()))
            verify(a.discInserted())
            tryCompare(a, "discState", "Seated", 5000)
            compare(a.insertionPending, false)
            verify(a.requestClose())
        } finally {
            app.destroy()
        }
    }

    function test_nonexistent_disc_recovers_to_absent() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const a = app.appliance
            verify(a.requestOpen())
            tryCompare(a, "lidState", "Open", 1200)
            verify(a.requestInsert("/tmp/vdisc-async-qml-nonexistent-destination.vdisc"))
            tryCompare(a, "discState", "Absent", 5000)
            compare(a.insertionPending, false)
            verify(a.machineError.length > 0)
            verify(a.lastRejection.length > 0)
        } finally {
            app.destroy()
        }
    }
}
