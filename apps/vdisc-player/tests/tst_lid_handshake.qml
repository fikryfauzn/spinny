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
            tryCompare(app, "active", true, 1000)
            keyClick(Qt.Key_O)
            compare(app.appliance.lidState, "Opening")
            tryCompare(app.appliance, "lidState", "Open", 1200)
            keyClick(Qt.Key_C)
            compare(app.appliance.lidState, "Closing")
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
            wait(170)
            const before = app.lidPivot.eulerRotation.x
            verify(before < -1)
            verify(!app.appliance.requestOpen())
            verify(!app.appliance.requestClose())
            compare(app.appliance.lidState, "Opening")
            verify(app.openingAnimation.running)
            verify(!app.closingAnimation.running)
            wait(80)
            verify(app.lidPivot.eulerRotation.x < before)
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
