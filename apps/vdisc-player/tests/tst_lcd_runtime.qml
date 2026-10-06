import QtQuick
import QtTest
import "../qml" as App

TestCase {
    name: "LcdRuntime"
    when: windowShown
    Component { id: appComponent; App.Main {} }
    function seated() {
        const app = createTemporaryObject(appComponent,null)
        verify(app !== null); verify(app.lcdSurface.assetReady)
        const a = app.appliance
        verify(a.requestOpen()); tryCompare(a,"lidState","Open",1000)
        verify(app.discDeck.selectDiscUrl(String(Qt.resolvedUrl("../../../tests/fixtures/vdisc/audio-runtime-two-track.vdisc"))))
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel,Qt.vector3d(-0.18,0.014,0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.05,0.014,0)))
        tryCompare(a,"discState","Seated",1000)
        verify(a.requestClose()); tryCompare(a,"lidState","Closed",1000)
        return app
    }
    function test_actual_position_pause_stop_resume_and_navigation() {
        const app = seated(); const a = app.appliance; const d = app.lcdSurface.display
        compare(d.primaryText,"02  00:04"); compare(d.statusText,"STOP")
        verify(a.requestPlay()); compare(d.primaryText,"01  00:00")
        audioTestDriver.setPosition(1234)
        tryCompare(d,"primaryText","01  00:01",500)
        verify(a.requestPause()); compare(d.statusText,"PAUSE")
        wait(100); compare(d.primaryText,"01  00:01")
        verify(a.requestPause()); verify(a.requestNext())
        compare(d.primaryText,"02  00:00")
        audioTestDriver.setPosition(1234); tryCompare(a,"lcdElapsedMs",1234,500)
        verify(a.requestPrevious()); compare(d.primaryText,"02  00:00")
        verify(a.requestPrevious()); compare(d.primaryText,"01  00:00")
        audioTestDriver.setPosition(1234); tryCompare(a,"lcdElapsedMs",1234,500)
        verify(a.requestStop()); compare(d.primaryText,"02  00:04")
        verify(a.requestPlay()); compare(d.primaryText,"01  00:01")
    }
    function test_eof_advances_then_final_stop_and_removal_clears() {
        const app = seated(); const a = app.appliance; const d = app.lcdSurface.display
        verify(a.requestPlay())
        audioTestDriver.queueEvent(1); tryCompare(d,"primaryText","02  00:00",500)
        audioTestDriver.queueEvent(1); tryCompare(d,"statusText","STOP",500)
        compare(d.primaryText,"02  00:04")
        verify(a.requestOpen()); tryCompare(a,"lidState","Open",1000)
        verify(app.discDeck.tryBeginDrag(app.discDeck.discModel,Qt.vector3d(0,0.014,0)))
        verify(app.discDeck.finishDragAt(Qt.vector3d(-0.18,0.014,0)))
        tryCompare(a,"discState","Absent",1000)
        compare(d.primaryText,"--  --:--")
    }
    function test_error_data() { return [{tag:"device",event:2,label:"AUDIO ERROR"},{tag:"decode",event:3,label:"PLAY ERROR"}] }
    function test_error(data) {
        const app = seated(); const a = app.appliance; const d = app.lcdSurface.display
        verify(a.requestToggleAvls()); verify(a.requestPlay()); verify(a.setHold(true))
        verify(!a.requestMenuShort()); compare(d.primaryText,"HOLD")
        audioTestDriver.queueEvent(data.event)
        tryCompare(d,"primaryText",data.label,500)
        compare(d.statusText,"STOP"); compare(d.holdText,"HOLD"); compare(d.avlsText,"AVLS")
        const rejection = a.lastRejection
        a.refreshLcdFeedback(); compare(a.lastRejection,rejection)
        compare(d.primaryText,data.label)
        tryCompare(a,"lcdFeedbackActive",false,2000)
        compare(d.primaryText,data.label)
    }
    function test_real_mode_and_indicator_projections() {
        const app = seated(); const a = app.appliance; const d = app.lcdSurface.display
        verify(a.requestPlay())
        for (const label of ["REP","1","REP 1","SHUF REP",""]) {
            verify(a.requestMenuShort()); compare(d.modeText,label)
        }
        verify(a.requestToggleAvls()); compare(d.avlsText,"AVLS")
        verify(a.setHold(true)); compare(d.holdText,"HOLD")
        verify(a.setVolume(0.2)); compare(d.primaryText,"01  00:00")
        verify(!a.requestOpen()); compare(d.primaryText,"HOLD")
        verify(a.setHold(false)); compare(d.primaryText,"01  00:00")
    }
}
