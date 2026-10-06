import QtQuick
import QtQuick3D
import QtTest
import "../qml" as App

TestCase {
    name: "NativeControlPicking"
    when: windowShown
    Component { id: appComponent; App.Main {} }
    function test_mesh_picking_and_pointer_routing() {
        if (GraphicsInfo.api === GraphicsInfo.Software || GraphicsInfo.api === GraphicsInfo.Unknown)
            skip("Requires native QRhi rendering; offscreen tests do not prove mesh picking")
        const app = createTemporaryObject(appComponent, null)
        tryCompare(app.contentItem, "width", app.width, 500)
        app.controls.windowActive = true
        wait(250)
        const surface = app.controls
        function point(model) { return surface.view3d.mapFrom3DScene(model.scenePosition) }
        function exposedPoint(model) {
            return model.objectName === "VOLUME_RAIL_TEST"
                ? surface.view3d.mapFrom3DScene(model.parent.mapPositionToScene(Qt.vector3d(0.035, 0.013, 0.073)))
                : point(model)
        }
        for (const name of surface.names.concat(surface.railNames)) {
            const model = surface.resolve(name)
            const p = exposedPoint(model)
            compare(surface.hitAt(p.x, p.y), model, "Picking " + name)
        }
        const hold = surface.resolve("HOLD_KNOB_TEST")
        let p = point(hold)
        mouseClick(surface, p.x, p.y)
        verify(app.appliance.holdEnabled)
        p = point(hold)
        mouseClick(surface, p.x, p.y)
        verify(!app.appliance.holdEnabled)
        const volume = surface.resolve("VOLUME_RAIL_TEST")
        p = exposedPoint(volume)
        mousePress(surface, p.x, p.y)
        compare(surface.captureKind, "Volume")
        const low = surface.view3d.mapFrom3DScene(volume.parent.mapPositionToScene(Qt.vector3d(0.02, 0.013, 0.0735)))
        mouseMove(surface, low.x, low.y)
        compare(app.appliance.volume, 0)
        mouseRelease(surface, low.x, low.y)
        compare(surface.captureKind, "")
        const menu = surface.buttonForName("BTN_MENU_TEST")
        p = point(menu)
        mousePress(surface, p.x, p.y)
        mouseMove(surface, 10, app.height - 10)
        mouseMove(surface, p.x, p.y)
        mouseRelease(surface, p.x, p.y)
        verify(!app.appliance.avlsEnabled)
        compare(surface.captureKind, "")
        app.inspection.windowActive = true
        verify(app.inspection.toggleLcdCloseup())
        wait(100)
        for (const name of ["BTN_MENU_TEST"].concat(surface.railNames)) {
            const model = surface.resolve(name)
            const projected = exposedPoint(model)
            compare(surface.hitAt(projected.x, projected.y), model, "Close-up picking " + name)
        }
        mousePress(surface, p.x, p.y)
        keyClick(Qt.Key_Escape)
        mouseRelease(surface, p.x, p.y)
        verify(!app.appliance.avlsEnabled)
        compare(surface.captureKind, "")
    }
}
