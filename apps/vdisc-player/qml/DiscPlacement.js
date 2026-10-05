.pragma library

function seatPose() {
    return Qt.vector3d(0, 0.014, 0)
}

function stagePose() {
    return Qt.vector3d(-0.18, 0.014, 0)
}

function localPath(fileUrl) {
    const url = String(fileUrl)
    if (!url.startsWith("file:///"))
        return null

    try {
        const path = decodeURIComponent(url.substring("file://".length))
        return path.startsWith("/") && /\.vdisc$/i.test(path) ? path : null
    } catch (error) {
        return null
    }
}

function finitePoint(point) {
    return point !== null && point !== undefined
            && Number.isFinite(point.x) && Number.isFinite(point.y)
            && Number.isFinite(point.z)
}

function intersectPlane(near, far, y) {
    if (!finitePoint(near) || !finitePoint(far) || !Number.isFinite(y))
        return null
    const dy = far.y - near.y
    if (Math.abs(dy) < 1e-6)
        return null
    const t = (y - near.y) / dy
    if (!Number.isFinite(t) || t < 0)
        return null
    const point = Qt.vector3d(near.x + t * (far.x - near.x), y,
                              near.z + t * (far.z - near.z))
    return finitePoint(point) ? point : null
}

function clampPreview(point) {
    if (!finitePoint(point))
        return null
    return Qt.vector3d(Math.max(-0.26, Math.min(0.10, point.x)), 0.014,
                       Math.max(-0.12, Math.min(0.12, point.z)))
}

function nearTarget(point, target, radius) {
    if (!finitePoint(point) || !finitePoint(target)
            || !Number.isFinite(radius) || radius < 0)
        return false
    const dx = point.x - target.x
    const dz = point.z - target.z
    return dx * dx + dz * dz <= radius * radius + 1e-12
}
