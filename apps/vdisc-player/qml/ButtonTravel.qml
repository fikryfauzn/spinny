import QtQuick
import QtQuick3D

Item {
    id: travel
    property bool initialized: false
    required property var model
    required property vector3d travelOffset
    property var boundModel: null
    property real restX: 0
    property real restY: 0
    property real restZ: 0
    property bool down: false
    readonly property bool ready: isLiveModel(model) && boundModel === model
    readonly property bool pressed: down
    readonly property vector3d restPosition: Qt.vector3d(restX, restY, restZ)

    function isLiveModel(object) {
        // QObject wrappers can remain truthy briefly after native destruction.
        try {
            return !!object && object instanceof Model
        } catch (error) {
            return false
        }
    }
    function syncModel() {
        motion.stop()
        down = false
        if (isLiveModel(boundModel))
            boundModel.position = restPosition
        boundModel = isLiveModel(model) ? model : null
        if (boundModel) {
            restX = boundModel.x
            restY = boundModel.y
            restZ = boundModel.z
        }
    }
    function moveTo(position, duration) {
        motion.stop()
        motion.from = Qt.vector3d(boundModel.x, boundModel.y, boundModel.z)
        motion.to = position
        motion.duration = duration
        motion.start()
    }
    function press() {
        if (!ready || down)
            return false
        down = true
        moveTo(Qt.vector3d(restX + travelOffset.x, restY + travelOffset.y,
                          restZ + travelOffset.z), 60)
        return true
    }
    function release() {
        down = false
        if (ready)
            moveTo(restPosition, 90)
    }
    onModelChanged: {
        if (initialized)
            syncModel()
    }
    Component.onCompleted: {
        initialized = true
        syncModel()
    }
    Component.onDestruction: {
        motion.stop()
        if (isLiveModel(boundModel))
            boundModel.position = restPosition
    }
    Vector3dAnimation {
        id: motion
        target: travel.boundModel
        property: "position"
        easing.type: Easing.InOutQuad
    }
    Binding {
        target: travel.model
        property: "pickable"
        value: true
        when: travel.ready
    }
}
