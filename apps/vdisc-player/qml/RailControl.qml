import QtQuick
import QtQuick3D

Item {
    id: rail
    required property var appliance
    required property var view3d
    required property var railModel
    required property var knobModel
    required property string kind
    property bool active: false
    property bool dragged: false
    property real startX: 0
    property real startY: 0
    property real grabOffset: 0
    property real previewX: 0
    readonly property real minimum: kind === "Hold" ? 0.017 : 0.032
    readonly property real maximum: kind === "Hold" ? 0.023 : 0.054
    readonly property bool assetReady: validModels()
    readonly property string hintText: !appliance ? "" : kind === "Hold"
        ? "HOLD: " + (appliance.holdEnabled ? "ON" : "OFF")
        : "VOLUME: " + Math.round(appliance.volume * 100) + "%"

    function validModels() {
        try {
            const prefix = kind === "Hold" ? "HOLD" : kind === "Volume" ? "VOLUME" : ""
            return prefix !== "" && railModel instanceof Model && knobModel instanceof Model
                && railModel.objectName === prefix + "_RAIL_TEST"
                && knobModel.objectName === prefix + "_KNOB_TEST"
                && knobModel.parent && knobModel.parent === railModel.parent
                && knobModel.parent.objectName === "PLAYER_ROOT"
        } catch (error) { return false }
    }
    function finite(point) {
        return point && Number.isFinite(point.x) && Number.isFinite(point.y) && Number.isFinite(point.z)
    }
    function intersectRay(near, far) {
        if (!finite(near) || !finite(far)) return null
        const dz = far.z - near.z
        if (Math.abs(dz) < 1e-9) return null
        const t = (0.0735 - near.z) / dz
        if (!Number.isFinite(t) || t < 0) return null
        const point = Qt.vector3d(near.x + t * (far.x - near.x),
                                  near.y + t * (far.y - near.y), 0.0735)
        return finite(point) ? point : null
    }
    function pointOnPlane(x, y) {
        if (!assetReady || !view3d || !view3d.camera || !Number.isFinite(x) || !Number.isFinite(y))
            return null
        const parent = knobModel.parent
        return intersectRay(parent.mapPositionFromScene(view3d.mapTo3DScene(Qt.vector3d(x, y, 0))),
                            parent.mapPositionFromScene(view3d.mapTo3DScene(Qt.vector3d(x, y, view3d.camera.clipFar))))
    }
    function clamp(x) { return Math.max(minimum, Math.min(maximum, x)) }
    function applyVolume(x) {
        return appliance.setVolume(Math.round((clamp(x) - minimum) / (maximum - minimum) * 100) / 100)
    }
    function begin(x, y, onKnob) {
        if (active) return false
        const point = pointOnPlane(x, y)
        if (!point) return false
        startX = x
        startY = y
        dragged = false
        grabOffset = onKnob ? point.x - knobModel.x : 0
        previewX = knobModel.x
        active = true
        if (kind === "Volume" && !onKnob && !applyVolume(point.x)) {
            cancel()
            return false
        }
        return true
    }
    function update(x, y) {
        if (!active) return false
        const point = pointOnPlane(x, y)
        if (!point) { cancel(); return false }
        if (Math.abs(x - startX) + Math.abs(y - startY) >= Application.styleHints.startDragDistance)
            dragged = true
        const target = clamp(point.x - grabOffset)
        if (kind === "Hold") {
            if (dragged) previewX = target
            return true
        }
        const accepted = applyVolume(target)
        if (!accepted) cancel()
        return accepted
    }
    function finish(x, y) {
        if (!active || !update(x, y)) return false
        const enabled = dragged ? previewX >= 0.020 - 1e-9 : !appliance.holdEnabled
        active = false
        return kind === "Hold" ? appliance.setHold(enabled) : true
    }
    function cancel() { active = false; dragged = false }
    onAssetReadyChanged: { if (!assetReady) cancel() }
    onRailModelChanged: cancel()
    onKnobModelChanged: cancel()
    onView3dChanged: cancel()
    Component.onDestruction: cancel()
    Binding {
        target: rail.knobModel
        property: "x"
        value: !rail.appliance ? rail.minimum : rail.kind === "Hold" ? (rail.active ? rail.previewX : rail.appliance.holdEnabled ? 0.023 : 0.017)
            : 0.032 + 0.022 * rail.appliance.volume
        when: rail.assetReady
    }
    Binding { target: rail.railModel; property: "pickable"; value: true; when: rail.assetReady }
    Binding { target: rail.knobModel; property: "pickable"; value: true; when: rail.assetReady }
}
