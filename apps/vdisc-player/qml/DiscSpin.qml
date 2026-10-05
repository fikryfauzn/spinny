import QtQuick
import QtQuick3D
import "DiscSpinPolicy.js" as Policy

Item {
    id: spin
    required property var appliance
    required property var sceneRoot
    property bool initialized: false
    property var boundModel: null
    property vector3d restRotation: Qt.vector3d(0, 0, 0)
    property real presentationAngle: 0
    property real presentationSpeed: 0
    property var effectiveTarget: null
    property int deliveredFrames: 0
    readonly property var candidate: resolveDisc()
    readonly property var discModel: assetReady ? boundModel : null
    readonly property bool assetReady: isLiveModel(candidate) && boundModel === candidate
    readonly property string assetError: assetReady ? "" : "Spin asset contract failure"
    readonly property real angle: presentationAngle
    readonly property real speed: presentationSpeed
    readonly property real targetSpeed: effectiveTarget === null ? 0 : effectiveTarget
    readonly property bool rampRunning: ramp.running
    readonly property bool frameRunning: frames.running
    readonly property int frameCount: deliveredFrames

    function isLiveModel(object) {
        try { return !!object && object instanceof Model }
        catch (error) { return false }
    }
    function unique(root, name) {
        let match = null
        let count = 0
        function visit(node) {
            if (!node)
                return
            if (node.objectName === name) { match = node; count++ }
            // Authored Models are leaves. Their runtime witness children depend
            // on this resolution, so traversing them creates a binding cycle.
            if (node instanceof Model)
                return
            for (const child of node.children)
                visit(child)
        }
        visit(root)
        return count === 1 ? match : null
    }
    function resolveDisc() {
        try {
            const player = unique(sceneRoot, "PLAYER_ROOT")
            const root = unique(sceneRoot, "DISC_ROOT")
            const model = unique(sceneRoot, "DISC_TEST")
            return player instanceof Node && !(player instanceof Model)
                && root instanceof Node && !(root instanceof Model)
                && isLiveModel(model) && root.parent === player && model.parent === root
                ? model : null
        } catch (error) { return null }
    }
    function stopImmediately() {
        ramp.stop()
        presentationSpeed = 0
    }
    function detach() {
        stopImmediately()
        if (isLiveModel(boundModel))
            boundModel.eulerRotation = restRotation
        boundModel = null
        presentationAngle = 0
    }
    function attach() {
        detach()
        if (isLiveModel(candidate)) {
            restRotation = candidate.eulerRotation
            boundModel = candidate
        }
        // A replacement starts at rest, even if the previous scene was playing.
        effectiveTarget = Policy.targetSpeed(appliance.transportState, appliance.lidState,
                                             appliance.discState, assetReady)
    }
    function syncPolicy() {
        if (!initialized)
            return
        const target = Policy.targetSpeed(appliance.transportState, appliance.lidState,
                                         appliance.discState, assetReady)
        if (target === null) {
            effectiveTarget = null
            stopImmediately()
        } else if (target !== effectiveTarget) {
            ramp.stop()
            effectiveTarget = target
            if (presentationSpeed !== target) {
                ramp.from = presentationSpeed
                ramp.to = target
                ramp.duration = target === 180 ? 400 : 350
                ramp.start()
            }
        }
        if (appliance.discState === "Absent") {
            presentationAngle = 0
            if (isLiveModel(boundModel))
                boundModel.eulerRotation = restRotation
        }
    }
    function advanceFrame(elapsedSeconds) {
        if (!assetReady) {
            stopImmediately()
            return
        }
        presentationAngle = Policy.advanceAngle(presentationAngle, presentationSpeed, elapsedSeconds)
        boundModel.eulerRotation = Qt.vector3d(restRotation.x,
            restRotation.y + presentationAngle, restRotation.z)
    }
    onCandidateChanged: { if (initialized) attach() }
    onAssetReadyChanged: { if (initialized && !assetReady) stopImmediately() }
    Component.onCompleted: { initialized = true; attach() }
    Component.onDestruction: detach()

    Connections {
        target: spin.appliance
        function onTransportStateChanged() { spin.syncPolicy() }
        function onLidStateChanged() { spin.syncPolicy() }
        function onDiscStateChanged() { spin.syncPolicy() }
    }
    NumberAnimation {
        id: ramp
        target: spin
        property: "presentationSpeed"
        easing.type: Easing.Linear
    }
    FrameAnimation {
        id: frames
        running: spin.assetReady && (spin.presentationSpeed > 0 || ramp.running)
        onTriggered: {
            spin.advanceFrame(frameTime)
            spin.deliveredFrames++
        }
    }
}
