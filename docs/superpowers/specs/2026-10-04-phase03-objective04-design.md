# Phase 3 Objective 4: Rust–QML Appliance Bridge

Status: implemented. This objective establishes the controller boundary; it does not implement presentation animation or audio output.

## Goal and ownership

QML must request appliance actions and observe their results without owning a second machine state. One Rust-owned CXX-Qt `QObject` wraps `De200Controller` and `CorePlayerBridge`. The controller decides action legality and owns lid, disc, transport, play-mode, HOLD, AVLS, volume, and machine-error state. `vdisc-core` remains the source of truth for loaded-disc metadata and playback position. QML owns only scene presentation and, in later objectives, animation and hit testing.

Success for Objective 4: a QML test creates the bridge, requests OPEN, and observes the lid projection change from `Closed` to `Opening`. An invalid request leaves controller state unchanged and exposes a rejection. No QML state machine or mechanical animation is added.

## Build boundary

Add a dedicated Rust static-library crate in the Cargo workspace for the CXX-Qt object. CMake imports its generated QML module and links it to `vdisc-player`. Keep the existing `VdiscPlayer` QML module and greybox asset intact. Use the CXX-Qt 0.10 CMake integration pattern and its version-matched Rust crates. Qt 6.8 remains the minimum. Resolve CXX-Qt build dependencies during implementation; do not vendor generated bindings.

## Bridge API

Expose one QML-creatable appliance object. Each instance owns one controller and one core-player bridge; `Main.qml` creates exactly one runtime instance. Initial volume is `0.5` (50%), supplied by the application, not added as a Phase 2 controller default. No audio sink is created here.

QML-invokable requests cover OPEN, close, insert path, remove, PLAY, PAUSE, STOP, PREVIOUS, NEXT, scan begin, scan end, scan step, MENU short, MENU long, HOLD set, AVLS toggle, and volume set. Presentation completion callbacks are `lidOpened`, `lidClosed`, `discInserted`, and `discRemoved`; they forward to existing controller completion methods. Scan step accepts an explicit target playback position; cadence and speed remain presentation policy for later work. Insert accepts a `.vdisc` path and uses `CorePlayerBridge` validation before the controller marks it seated.

Requests return success/failure to QML. A rejected request reports a read-only `lastRejection` string separately from the controller's machine error. Rejection never fabricates machine state. Clear `lastRejection` on a successful request. Invalid numeric or path input is rejected at the bridge boundary without panicking.

The read-only projection includes lid, disc, and transport states; play mode; HOLD and AVLS; volume; application gain; machine error; and LCD fields. Represent enum states with stable documented strings and numeric values with explicit units. QML has no setters for these fields. Project a fresh snapshot and emit change notifications only after each request or completion, using controller and backend getters; do not cache another mutable state machine in QML or the bridge. LCD data comes from the existing `lcd_snapshot` API and backend facts.

## Runtime and error flow

QML calls a bridge request. The bridge validates QML input, invokes the existing controller or core-player operation, then refreshes its projection. Successful OPEN publishes `Opening`; only the later animation completion callback can publish `Open`. A rejected PLAY with no disc, or OPEN during playback, leaves mechanical and transport state unchanged and reports the rejection. Backend failures use existing controller/core translation; the bridge never silently reports success when the backend fails.

For Objective 4, `Main.qml` instantiates the bridge but does not attach buttons or animate assets. Objectives 5–8 connect physical interaction and completion callbacks. Objective 9 connects audio output. The bridge's API must not assume a particular visual timing or audio device.

## Verification

- Rust tests check initial projection, representative accepted and rejected requests, completion callbacks, gain/volume projection, and preservation of controller/backend ownership.
- A Qt Quick test creates the runtime object through its imported QML module, requests OPEN, verifies `Opening`, calls `lidOpened`, and verifies `Open`. A rejection test verifies unchanged state and a populated rejection.
- Build Cargo workspace and CMake Qt app, run Rust and QML tests, and lint changed QML. If dependency fetching or a GUI tool is unavailable, report the exact gate rather than claim it passed.

## Out of scope

No lid/disc animations, pointer hit testing, final D-E200 appearance, persisted volume, scan cadence, or real audio output. These belong to later Phase 3 objectives.

## Documentation basis

- [CXX-Qt CMake integration](https://kdab.github.io/cxx-qt/book/getting-started/5-cmake-integration.html)
- [CXX-Qt QObject properties and invokables](https://kdab.github.io/cxx-qt/book/bridge/extern_rustqt.html)
