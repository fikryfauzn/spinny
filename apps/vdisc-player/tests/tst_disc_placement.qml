import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App
import "../qml/DiscPlacement.js" as Placement

TestCase {
    name: "DiscPlacement"

    Component {
        id: appComponent
        App.Main {}
    }

    Component {
        id: duplicateRootComponent
        Node {
            Node { objectName: "DISC_ROOT" }
            Node { objectName: "DISC_ROOT" }
            Model { objectName: "DISC_TEST" }
            Model { objectName: "DISC_TEST" }
            Model { objectName: "SPINDLE_TEST" }
            Model { objectName: "SPINDLE_TEST" }
        }
    }

    function validDiscPath() {
        const url = String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
        return decodeURIComponent(url.substring("file://".length))
    }

    function validDiscUrl() {
        return String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/valid-v1.vdisc"))
    }

    function test_staging_marker_tracks_view_resize() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            wait(20)
            const beforeX = deck.stageScreen.x
            const beforeY = deck.stageScreen.y
            app.width = 1200
            app.height = 850
            wait(20)
            const expected = deck.view3d.mapFrom3DScene(
                deck.discRoot.parent.mapPositionToScene(Qt.vector3d(-0.18, 0.014, 0)))
            verify(Math.abs(expected.x - beforeX) + Math.abs(expected.y - beforeY) > 1)
            fuzzyCompare(deck.stageScreen.x, expected.x, 0.1)
            fuzzyCompare(deck.stageScreen.y, expected.y, 0.1)
        } finally {
            app.destroy()
        }
    }

    function test_authoritative_transition_cancels_active_drag() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.updateDrag(Qt.vector3d(-0.1, 0.014, 0)))
            verify(app.appliance.requestInsert(validDiscPath()))
            compare(app.appliance.discState, "Inserting")
            const position = deck.discRoot.position
            compare(deck.dragging, false)
            verify(!deck.updateDrag(Qt.vector3d(0, 0.014, 0)))
            compare(deck.discRoot.position, position)
        } finally {
            app.destroy()
        }
    }

    function test_asset_loss_during_settle_data() {
        return [{ tag: "insertion", removing: false },
                { tag: "removal", removing: true }]
    }

    function test_asset_loss_during_settle(data) {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
            if (data.removing) {
                tryCompare(app.appliance, "discState", "Seated", 1000)
                verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(0, 0.014, 0)))
                verify(deck.finishDragAt(Qt.vector3d(-0.12, 0.014, 0)))
            }
            const state = data.removing ? "Removing" : "Inserting"
            compare(app.appliance.discState, state)
            verify(data.removing ? deck.removalAnimation.running : deck.insertionAnimation.running)
            deck.discModel.objectName = "RENAMED_DISC_TEST"
            tryVerify(function() { return !deck.assetReady }, 100)
            wait(450)
            compare(app.appliance.discState, state)
        } finally {
            app.destroy()
        }
    }

    function test_disc_starts_hidden_and_nodes_are_unique() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck !== undefined && deck !== null)
            verify(deck.assetReady)
            compare(app.appliance.discState, "Absent")
            compare(deck.selectedPath, "")
            compare(deck.discRoot.visible, false)
            compare(deck.discRoot.parent.objectName, "PLAYER_ROOT")
            compare(deck.discModel.parent, deck.discRoot)
            compare(deck.spindleModel.parent, deck.discRoot.parent)
            fuzzyCompare(deck.discRoot.position.y, 0.014, 0.0001)
        } finally {
            app.destroy()
        }
    }

    function test_staging_waits_for_camera_before_projection() {
        failOnWarning(/Cannot resolve view position without a camera assigned/)
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            verify(app.discDeck.assetReady)
        } finally {
            app.destroy()
        }
    }

    function test_local_url_decodes_and_stages_disc() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl("file:///tmp/a%20b%23%25%E9%9B%AA.vdisc"))
            compare(deck.selectedPath, "/tmp/a b#%雪.vdisc")
            compare(deck.discRoot.visible, true)
            fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            fuzzyCompare(deck.discRoot.position.y, 0.014, 0.0001)
            fuzzyCompare(deck.discRoot.position.z, 0, 0.0001)
            compare(app.appliance.discState, "Absent")
        } finally {
            app.destroy()
        }
    }

    function test_bad_file_choices_preserve_staged_disc() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl("file:///tmp/first.vdisc"))
            const bad = ["", "file:///tmp/bad%ZZ.vdisc", "https://example.com/a.vdisc",
                         "file://remote/tmp/a.vdisc", "file:///tmp/audio.mp3"]
            for (const url of bad) {
                verify(!deck.selectDiscUrl(url), url)
                compare(deck.selectedPath, "/tmp/first.vdisc")
                fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            }
            verify(deck.selectDiscUrl("file:///tmp/second.vdisc"))
            compare(deck.selectedPath, "/tmp/second.vdisc")
        } finally {
            app.destroy()
        }
    }

    function test_selection_cannot_replace_inserting_or_seated_disc() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl("file:///tmp/staged.vdisc"))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(app.appliance.requestInsert(validDiscPath()), app.appliance.lastRejection)
            compare(app.appliance.discState, "Inserting")
            verify(!deck.selectDiscUrl("file:///tmp/other.vdisc"))
            compare(deck.selectedPath, "/tmp/staged.vdisc")
            verify(app.appliance.discInserted())
            compare(app.appliance.discState, "Seated")
            verify(!deck.selectDiscUrl("file:///tmp/other.vdisc"))
            compare(deck.selectedPath, "/tmp/staged.vdisc")
        } finally {
            app.destroy()
        }
    }

    function test_missing_or_duplicate_disc_nodes_fail_closed() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        const duplicates = duplicateRootComponent.createObject(null)
        verify(duplicates !== null)
        try {
            const names = ["DISC_ROOT", "DISC_TEST", "SPINDLE_TEST"]
            for (const name of names) {
                verify(app.findUniqueNamedNode(app.greybox, name) !== null)
                compare(app.findUniqueNamedNode(app.greybox, "MISSING_" + name), null)
                compare(app.findUniqueNamedNode(duplicates, name), null)
            }
            verify(app.discDeck.assetReady)
            app.discDeck.discModel.objectName = "RENAMED_DISC_TEST"
            tryVerify(function() { return !app.discDeck.assetReady }, 100)
        } finally {
            duplicates.destroy()
            app.destroy()
        }
    }

    function test_drag_plane_rejects_bad_rays_and_clamps_preview() {
        const crossing = Placement.intersectPlane(Qt.vector3d(0, 1, 0),
                                                  Qt.vector3d(2, -1, 2), 0.014)
        verify(crossing !== null)
        fuzzyCompare(crossing.x, 0.986, 0.0001)
        fuzzyCompare(crossing.y, 0.014, 0.0001)
        fuzzyCompare(crossing.z, 0.986, 0.0001)
        compare(Placement.intersectPlane(Qt.vector3d(0, 1, 0),
                                         Qt.vector3d(2, 1, 2), 0.014), null)
        compare(Placement.intersectPlane(Qt.vector3d(NaN, 1, 0),
                                         Qt.vector3d(2, -1, 2), 0.014), null)
        const bounded = Placement.clampPreview(Qt.vector3d(-99, 4, 99))
        fuzzyCompare(bounded.x, -0.26, 0.0001)
        fuzzyCompare(bounded.y, 0.014, 0.0001)
        fuzzyCompare(bounded.z, 0.12, 0.0001)
        compare(Placement.clampPreview(Qt.vector3d(NaN, 4, 0)), null)
        verify(Placement.nearTarget(Qt.vector3d(0.06, 0.014, 0),
                                    Qt.vector3d(0, 0.014, 0), 0.06))
        verify(!Placement.nearTarget(Qt.vector3d(0.0601, 0.014, 0),
                                     Qt.vector3d(0, 0.014, 0), 0.06))
    }

    function test_open_lid_disc_drag_moves_only_root_and_miss_snaps_back() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl("file:///tmp/staged.vdisc"))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            const scale = deck.discRoot.scale
            const rotation = deck.discRoot.eulerRotation
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.updateDrag(Qt.vector3d(-0.09, 0.014, 0.04)))
            fuzzyCompare(deck.discRoot.position.x, -0.09, 0.0001)
            fuzzyCompare(deck.discRoot.position.y, 0.014, 0.0001)
            fuzzyCompare(deck.discRoot.position.z, 0.04, 0.0001)
            compare(deck.discRoot.scale, scale)
            compare(deck.discRoot.eulerRotation, rotation)
            verify(!deck.finishDragAt(Qt.vector3d(-0.1, 0.014, 0.1)))
            compare(app.appliance.discState, "Absent")
            fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            fuzzyCompare(deck.discRoot.position.z, 0, 0.0001)
        } finally {
            app.destroy()
        }
    }

    function test_drag_guards_wrong_model_lid_and_transition() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            const stage = Qt.vector3d(-0.18, 0.014, 0)
            verify(!deck.tryBeginDrag(deck.discModel, stage))
            verify(deck.selectDiscUrl("file:///tmp/staged.vdisc"))
            verify(!deck.tryBeginDrag(deck.discModel, stage))
            verify(app.appliance.requestOpen())
            compare(app.appliance.lidState, "Opening")
            verify(!deck.tryBeginDrag(deck.discModel, stage))
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(!deck.tryBeginDrag(deck.spindleModel, stage))
            verify(deck.tryBeginDrag(deck.discModel, stage))
            deck.cancelDrag()
            verify(app.appliance.requestInsert(validDiscPath()))
            compare(app.appliance.discState, "Inserting")
            verify(!deck.tryBeginDrag(deck.discModel, stage))
        } finally {
            app.destroy()
        }
    }

    function test_closing_lid_cancels_loose_disc_drag() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl("file:///tmp/staged.vdisc"))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.updateDrag(Qt.vector3d(-0.1, 0.014, 0)))
            verify(app.appliance.requestClose())
            compare(app.appliance.lidState, "Closing")
            fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            compare(app.appliance.discState, "Absent")
            verify(!deck.finishDragAt(Qt.vector3d(0, 0.014, 0)))
        } finally {
            app.destroy()
        }
    }

    function test_valid_disc_seats_and_removes_only_after_motion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
            compare(app.appliance.discState, "Inserting")
            verify(deck.insertionAnimation.running)
            tryVerify(function() { return deck.discRoot.position.x > -0.04 }, 250)
            compare(app.appliance.discState, "Inserting")
            tryCompare(app.appliance, "discState", "Seated", 1000)
            fuzzyCompare(deck.discRoot.position.x, 0, 0.0001)
            fuzzyCompare(deck.discRoot.position.y, 0.014, 0.0001)

            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(0, 0.014, 0)))
            verify(!deck.finishDragAt(Qt.vector3d(0.1, 0.014, 0.1)))
            compare(app.appliance.discState, "Seated")
            fuzzyCompare(deck.discRoot.position.x, 0, 0.0001)

            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(0, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.12, 0.014, 0)))
            compare(app.appliance.discState, "Removing")
            verify(deck.removalAnimation.running)
            tryVerify(function() { return deck.discRoot.position.x < -0.13 }, 250)
            compare(app.appliance.discState, "Removing")
            tryCompare(app.appliance, "discState", "Absent", 1000)
            fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            compare(deck.selectedPath, validDiscPath())
            verify(deck.discRoot.visible)
        } finally {
            app.destroy()
        }
    }

    function test_invalid_package_and_closed_lid_never_start_seating() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(!deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(!app.appliance.requestInsert(validDiscPath()))
            verify(app.appliance.lastRejection.length > 0)
            compare(app.appliance.discState, "Absent")
            verify(!deck.insertionAnimation.running)

            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.selectDiscUrl("file:///tmp/objective06-missing.vdisc"))
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(!deck.finishDragAt(Qt.vector3d(0, 0.014, 0)))
            compare(app.appliance.discState, "Absent")
            verify(!deck.insertionAnimation.running)
            verify(app.appliance.lastRejection.length > 0 || app.appliance.machineError.length > 0)
            fuzzyCompare(deck.discRoot.position.x, -0.18, 0.0001)
            compare(deck.selectedPath, "/tmp/objective06-missing.vdisc")
        } finally {
            app.destroy()
        }
    }

    function test_stopped_insertion_cannot_report_seated() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
            wait(100)
            deck.insertionAnimation.stop()
            compare(app.appliance.discState, "Inserting")
            wait(450)
            compare(app.appliance.discState, "Inserting")
            verify(!deck.insertionAnimation.running)
        } finally {
            app.destroy()
        }
    }

    function test_stopped_removal_cannot_report_absent() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(0, 0.014, 0)))
            tryCompare(app.appliance, "discState", "Seated", 1000)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(0, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.12, 0.014, 0)))
            wait(100)
            deck.removalAnimation.stop()
            compare(app.appliance.discState, "Removing")
            wait(450)
            compare(app.appliance.discState, "Removing")
            verify(!deck.removalAnimation.running)
        } finally {
            app.destroy()
        }
    }

    function test_missing_scene_node_cannot_complete_external_insert() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            deck.discModel.objectName = "RENAMED_DISC_TEST"
            tryVerify(function() { return !deck.assetReady }, 100)
            verify(app.appliance.requestInsert(validDiscPath()))
            compare(app.appliance.discState, "Inserting")
            verify(!deck.insertionAnimation.running)
            wait(450)
            compare(app.appliance.discState, "Inserting")
        } finally {
            app.destroy()
        }
    }

    function test_rejected_transition_requests_do_not_restart_motion() {
        const app = appComponent.createObject(null)
        verify(app !== null)
        try {
            const deck = app.discDeck
            verify(deck.selectDiscUrl(validDiscUrl()))
            verify(app.appliance.requestOpen())
            tryCompare(app.appliance, "lidState", "Open", 1200)
            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(-0.18, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.05, 0.014, 0)))
            wait(100)
            const beforeInsert = deck.discRoot.position.x
            verify(!app.appliance.requestInsert(validDiscPath()))
            verify(!app.appliance.requestRemove())
            compare(app.appliance.discState, "Inserting")
            verify(deck.insertionAnimation.running)
            wait(80)
            verify(deck.discRoot.position.x > beforeInsert)
            tryCompare(app.appliance, "discState", "Seated", 1000)

            verify(deck.tryBeginDrag(deck.discModel, Qt.vector3d(0, 0.014, 0)))
            verify(deck.finishDragAt(Qt.vector3d(-0.12, 0.014, 0)))
            wait(100)
            const beforeRemove = deck.discRoot.position.x
            verify(!app.appliance.requestRemove())
            verify(!app.appliance.requestInsert(validDiscPath()))
            compare(app.appliance.discState, "Removing")
            verify(deck.removalAnimation.running)
            wait(80)
            verify(deck.discRoot.position.x < beforeRemove)
            tryCompare(app.appliance, "discState", "Absent", 1000)
        } finally {
            app.destroy()
        }
    }
}
