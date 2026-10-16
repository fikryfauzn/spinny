import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "LidHandshake"

    Component {
        id: appComponent
        App.Main {}
    }

    Component {
        id: duplicateRootComponent
        Node {
            Node { objectName: "LID_ROOT" }
            Node { objectName: "LID_ROOT" }
        }
    }

    function test_lid_lookup_requires_one_named_node() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        const duplicates = duplicateRootComponent.createObject(null)
        verify(duplicates !== null)
        try {
            verify(app.lidPivot !== undefined)
            compare(app.findUniqueNamedNode(app.greybox, "LID_ROOT"), app.lidPivot)
            compare(app.findUniqueNamedNode(app.greybox, "MISSING_LID_ROOT"), null)
            compare(app.findUniqueNamedNode(duplicates, "LID_ROOT"), null)
            compare(app.lidPivot.eulerRotation.x, 0)
        } finally {
            duplicates.destroy()
            app.destroy()
        }
    }

    function test_open_moves_hinge_then_reports_completion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.lidPivot !== undefined && app.lidPivot !== null)
            const pivot = app.lidPivot
            const position = pivot.position
            const scale = pivot.scale
            const y = pivot.eulerRotation.y
            const z = pivot.eulerRotation.z

            compare(app.appliance.lidState, "Closed")
            verify(app.appliance.requestOpen())
            compare(app.appliance.lidState, "Opening")
            verify(app.openingAnimation.running)
            tryVerify(function() { return pivot.eulerRotation.x < -1 }, 400)
            compare(app.appliance.lidState, "Opening")
            tryCompare(app.appliance, "lidState", "Open", 1200)
            fuzzyCompare(pivot.eulerRotation.x, -105, 0.1)
            compare(pivot.eulerRotation.y, y)
            compare(pivot.eulerRotation.z, z)
            compare(pivot.position, position)
            compare(pivot.scale, scale)
        } finally {
            app.destroy()
        }
    }

    function test_close_moves_hinge_then_reports_completion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(app.appliance.requestClose())
            compare(app.appliance.lidState, "Closing")
            verify(app.closingAnimation.running)
            tryVerify(function() { return app.lidPivot.eulerRotation.x > -104 }, 400)
            compare(app.appliance.lidState, "Closing")
            tryCompare(app.appliance, "lidState", "Closed", 1200)
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
        } finally {
            app.destroy()
        }
    }

    function test_stopped_open_does_not_report_completion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.lidPivot !== undefined && app.lidPivot !== null)
            verify(app.appliance.requestOpen())
            tryVerify(function() { return app.lidPivot.eulerRotation.x < -1 }, 400)
            app.openingAnimation.stop()
            compare(app.appliance.lidState, "Opening")
            wait(650)
            compare(app.appliance.lidState, "Opening")
            verify(!app.openingAnimation.running)
        } finally {
            app.destroy()
        }
    }

    function test_stopped_close_does_not_report_completion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(app.appliance.requestClose())
            tryVerify(function() { return app.lidPivot.eulerRotation.x > -104 }, 400)
            app.closingAnimation.stop()
            compare(app.appliance.lidState, "Closing")
            wait(650)
            compare(app.appliance.lidState, "Closing")
            verify(!app.closingAnimation.running)
        } finally {
            app.destroy()
        }
    }

    function test_missing_pivot_cannot_complete_open() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.lidPivot !== undefined && app.lidPivot !== null)
            app.lidPivot.objectName = "RENAMED_LID_ROOT"
            tryVerify(function() { return app.lidPivot === null }, 100)
            verify(app.appliance.requestOpen())
            compare(app.appliance.lidState, "Opening")
            verify(!app.openingAnimation.running)
            wait(650)
            compare(app.appliance.lidState, "Opening")
        } finally {
            app.destroy()
        }
    }

    function test_keyboard_requests_open_and_close() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            app.requestActivate()
            tryCompare(app, "active", true, 2000)
            tryCompare(app.controls, "inputActive", true, 1000)
            keyClick(Qt.Key_O)
            tryCompare(app.appliance, "lidState", "Opening", 1000)
            tryCompare(app.appliance, "lidState", "Open", 1200)

            // QML TestCase.keyClick() targets the active window. During the
            // opening animation another test window can take focus. The C
            // shortcut is intentionally disabled when this window is inactive,
            // so reacquire activation before asserting its keyboard behavior.
            app.requestActivate()
            tryCompare(app, "active", true, 2000)
            tryCompare(app.controls, "inputActive", true, 1000)
            keyClick(Qt.Key_C)
            tryCompare(app.appliance, "lidState", "Closing", 1000)
            tryCompare(app.appliance, "lidState", "Closed", 1200)
        } finally {
            app.destroy()
        }
    }

    function test_playing_rejects_open_without_motion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
            const path = decodeURIComponent(url.substring("file://".length))
            verify(app.appliance.requestInsert(path), app.appliance.lastRejection)
            verify(app.appliance.discInserted())
            tryCompare(app.appliance, "discState", "Seated", 5000)
            verify(app.appliance.requestClose())
            tryCompare(app.appliance, "lidState", "Closed", 1200)
            verify(app.appliance.requestPlay())
            compare(app.appliance.transportState, "Playing")

            verify(!app.appliance.requestOpen())
            verify(app.appliance.lastRejection.length > 0)
            compare(app.appliance.lidState, "Closed")
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
            verify(!app.openingAnimation.running)
            verify(!app.closingAnimation.running)
            wait(100)
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
        } finally {
            app.destroy()
        }
    }

    function test_hold_rejects_open_without_motion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.setHold(true))
            verify(!app.appliance.requestOpen())
            verify(app.appliance.lastRejection.length > 0)
            compare(app.appliance.lidState, "Closed")
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
            verify(!app.openingAnimation.running)
            verify(!app.closingAnimation.running)
            wait(100)
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
        } finally {
            app.destroy()
        }
    }

    function test_rejected_requests_do_not_restart_opening() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.requestOpen())
            // Wait for real movement, not an assumed frame after 170 ms.
            tryVerify(function() {
                return app.openingAnimation.running
                        && app.appliance.lidState === "Opening"
                        && app.lidPivot.eulerRotation.x < -1
            }, 1800)
            // Freeze the in-flight animation so the rejection test does not
            // race its natural 600 ms completion on a busy render thread.
            app.openingAnimation.pause()
            const before = app.lidPivot.eulerRotation.x
            verify(before < -1)
            verify(!app.appliance.requestOpen())
            verify(!app.appliance.requestClose())
            compare(app.appliance.lidState, "Opening")
            verify(app.openingAnimation.running)
            verify(app.openingAnimation.paused)
            verify(!app.closingAnimation.running)
            fuzzyCompare(app.lidPivot.eulerRotation.x, before, 0.1)
            app.openingAnimation.resume()
            tryCompare(app.appliance, "lidState", "Open", 1200)
            fuzzyCompare(app.lidPivot.eulerRotation.x, -105, 0.1)
        } finally {
            app.destroy()
        }
    }

    function test_rejected_requests_do_not_restart_closing() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(app.appliance.requestClose())
            wait(170)
            const before = app.lidPivot.eulerRotation.x
            verify(before > -104)
            verify(!app.appliance.requestClose())
            verify(!app.appliance.requestOpen())
            compare(app.appliance.lidState, "Closing")
            verify(!app.openingAnimation.running)
            verify(app.closingAnimation.running)
            wait(80)
            verify(app.lidPivot.eulerRotation.x > before)
            tryCompare(app.appliance, "lidState", "Closed", 1200)
            fuzzyCompare(app.lidPivot.eulerRotation.x, 0, 0.1)
        } finally {
            app.destroy()
        }
    }
}
