.pragma library

function targetSpeed(transport, lid, disc, assetReady) {
    if (!assetReady || lid !== "Closed" || disc !== "Seated")
        return null
    if (transport === "Playing" || transport === "SeekingForward"
            || transport === "SeekingBackward")
        return 180
    if (transport === "Paused" || transport === "Stopped")
        return 0
    return null
}

function advanceAngle(angle, speed, elapsedSeconds) {
    const start = Number.isFinite(angle) ? angle : 0
    const elapsed = Number.isFinite(elapsedSeconds) && elapsedSeconds > 0
        ? Math.min(elapsedSeconds, 0.05) : 0
    const delta = Number.isFinite(speed) ? speed * elapsed : 0
    return ((start + delta) % 360 + 360) % 360
}
