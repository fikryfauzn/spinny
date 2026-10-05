import QtQuick
import QtQuick.Dialogs
import "DiscPlacement.js" as Placement

Item {
    id: deck

    required property var view3d
    required property var appliance
    required property var discRoot
    required property var discModel
    required property var spindleModel
    readonly property alias insertionAnimation: insertMotion
    readonly property alias removalAnimation: removeMotion
    property string selectedPath: ""
    property bool dragging: false
    property vector3d dragOffset: Qt.vector3d(0, 0, 0)
    readonly property bool assetReady: discRoot !== null && discModel !== null
                                       && spindleModel !== null
                                       && discModel.parent === discRoot
                                       && spindleModel.parent === discRoot.parent
    readonly property vector3d stageScreen: {
        if (!assetReady || !view3d || !view3d.camera
                || view3d.width <= 0 || view3d.height <= 0)
            return Qt.vector3d(0, 0, 0)
        return view3d.mapFrom3DScene(
            discRoot.parent.mapPositionToScene(Placement.stagePose()))
    }
    readonly property string statusText: !assetReady ? "Disc asset contract failure"
        : appliance.lastRejection !== "" ? appliance.lastRejection
        : appliance.machineError !== "" ? appliance.machineError
        : selectedPath !== "" ? selectedPath
        : "Choose a .vdisc, then open the lid"

    function selectDiscUrl(fileUrl) {
        if (!assetReady || appliance.discState !== "Absent")
            return false
        const path = Placement.localPath(fileUrl)
        if (path === null)
            return false
        selectedPath = path
        discRoot.position = Placement.stagePose()
        return true
    }

    function pointOnPlane(x, y) {
        if (!assetReady || !view3d || !view3d.camera)
            return null
        const near = discRoot.parent.mapPositionFromScene(
            view3d.mapTo3DScene(Qt.vector3d(x, y, 0)))
        const far = discRoot.parent.mapPositionFromScene(
            view3d.mapTo3DScene(Qt.vector3d(x, y, 1)))
        return Placement.intersectPlane(near, far, 0.014)
    }

    function tryBeginDrag(hitObject, localPoint) {
        if (dragging || !assetReady || hitObject !== discModel
                || appliance.lidState !== "Open"
                || (appliance.discState !== "Absent" && appliance.discState !== "Seated")
                || (appliance.discState === "Absent" && selectedPath === "")
                || Placement.clampPreview(localPoint) === null)
            return false
        dragOffset = Qt.vector3d(discRoot.position.x - localPoint.x, 0,
                                 discRoot.position.z - localPoint.z)
        dragging = true
        return true
    }

    function updateDrag(localPoint) {
        if (!dragging)
            return false
        if (!assetReady || appliance.lidState !== "Open"
                || (appliance.discState !== "Absent" && appliance.discState !== "Seated")) {
            cancelDrag()
            return false
        }
        if (localPoint === null || localPoint === undefined) {
            cancelDrag()
            return false
        }
        const preview = Placement.clampPreview(Qt.vector3d(
            localPoint.x + dragOffset.x, 0.014, localPoint.z + dragOffset.z))
        if (preview === null) {
            cancelDrag()
            return false
        }
        discRoot.position = preview
        return true
    }

    function snapToProjectedPose() {
        if (!assetReady)
            return
        if (appliance.discState === "Absent")
            discRoot.position = Placement.stagePose()
        else if (appliance.discState === "Seated")
            discRoot.position = Placement.seatPose()
    }

    function cancelDrag() {
        if (!dragging)
            return
        dragging = false
        snapToProjectedPose()
    }

    function finishDragAt(localPoint) {
        if (!dragging || !updateDrag(localPoint))
            return false
        dragging = false

        if (appliance.discState === "Absent"
                && Placement.nearTarget(discRoot.position, Placement.seatPose(), 0.06)) {
            if (appliance.requestInsert(selectedPath)
                    && appliance.discState === "Inserting" && assetReady) {
                insertMotion.from = discRoot.position
                insertMotion.to = Placement.seatPose()
                insertMotion.start()
                return true
            }
        } else if (appliance.discState === "Seated"
                   && Placement.nearTarget(discRoot.position, Placement.stagePose(), 0.075)) {
            if (appliance.requestRemove()
                    && appliance.discState === "Removing" && assetReady) {
                removeMotion.from = discRoot.position
                removeMotion.to = Placement.stagePose()
                removeMotion.start()
                return true
            }
        }
        snapToProjectedPose()
        return false
    }

    Vector3dAnimation {
        id: insertMotion
        target: deck.discRoot
        property: "position"
        duration: 400
        easing.type: Easing.InOutQuad
        onFinished: {
            if (deck.assetReady && deck.appliance.discState === "Inserting")
                deck.appliance.discInserted()
        }
    }

    Vector3dAnimation {
        id: removeMotion
        target: deck.discRoot
        property: "position"
        duration: 400
        easing.type: Easing.InOutQuad
        onFinished: {
            if (deck.assetReady && deck.appliance.discState === "Removing")
                deck.appliance.discRemoved()
        }
    }

    Connections {
        target: deck.appliance
        function onDiscStateChanged() {
            if (deck.appliance.discState !== "Absent"
                    && deck.appliance.discState !== "Seated")
                deck.cancelDrag()
        }
        function onLidStateChanged() {
            if (deck.appliance.lidState !== "Open")
                deck.cancelDrag()
        }
    }

    Binding {
        target: deck.discRoot
        property: "visible"
        value: deck.assetReady
               && (deck.appliance.discState !== "Absent" || deck.selectedPath !== "")
        when: deck.discRoot !== null
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onPressed: function(mouse) {
            if (!deck.assetReady || deck.appliance.lidState !== "Open") {
                mouse.accepted = false
                return
            }
            const point = deck.pointOnPlane(mouse.x, mouse.y)
            const hit = deck.view3d.pick(mouse.x, mouse.y)
            mouse.accepted = point !== null && deck.tryBeginDrag(hit.objectHit, point)
        }
        onPositionChanged: function(mouse) {
            if (deck.dragging)
                deck.updateDrag(deck.pointOnPlane(mouse.x, mouse.y))
        }
        onReleased: function(mouse) {
            if (deck.dragging)
                deck.finishDragAt(deck.pointOnPlane(mouse.x, mouse.y))
        }
        onCanceled: deck.cancelDrag()
    }

    Binding {
        target: deck.discModel
        property: "pickable"
        value: deck.assetReady
        when: deck.discModel !== null
    }

    Rectangle {
        id: stageMarker
        visible: deck.assetReady && deck.view3d && deck.view3d.camera
        width: 80
        height: 80
        radius: 40
        x: deck.stageScreen.x - width / 2
        y: deck.stageScreen.y - height / 2
        color: "transparent"
        border.color: "#99c6d5df"
        border.width: 2
    }

    Rectangle {
        id: chooseButton
        width: 150
        height: 38
        x: 16
        y: 16
        radius: 5
        color: "#35414c"
        border.color: "#a8b9c5"

        Text {
            anchors.centerIn: parent
            text: "Choose .vdisc"
            color: "white"
        }

        MouseArea {
            anchors.fill: parent
            onClicked: discFileDialog.open()
        }
    }

    Text {
        x: 16
        y: chooseButton.y + chooseButton.height + 8
        width: Math.min(parent.width - 32, 500)
        color: "#e1e9ee"
        wrapMode: Text.Wrap
        text: deck.statusText
    }

    FileDialog {
        id: discFileDialog
        title: "Choose a VDISC"
        nameFilters: ["VDISC discs (*.vdisc)"]
        onAccepted: deck.selectDiscUrl(selectedFile)
    }
}
