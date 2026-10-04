# Phase 3 Objective 4: Rust–QML Appliance Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** QML requests appliance actions through one Rust-owned object and observes controller-owned state changes.

**Architecture:** A pure Rust runtime routes commands to `De200Controller` and `CorePlayerBridge` and projects an immutable snapshot. A CXX-Qt `QObject` exposes invokables and read-only, notified properties. The existing Qt app creates one object; animation and audio remain later objectives.

**Tech Stack:** Rust 2024, `vdisc-appliance`, CXX-Qt 0.10, Qt Quick 6.8+, CMake 3.24+, Qt Quick Test.

**Spec:** `docs/superpowers/specs/2026-10-04-phase03-objective04-design.md`

**Build note:** The Cargo package is named `vdisc_qml_bridge` (underscores) so its
CXX-Qt CMake export matches the Rust crate target. The static QML module requires
the linked `vdisc-appliance-tests` runner rather than the system `qmltestrunner`.
Use `-import /tmp/vdisc-phase03-obj04-build/cxxqt/qml_modules` with that runner.
The CMake build also packages both core validation workers beside the executables.

## Global Constraints

- Initial volume: `0.5` normalized; no persisted-volume policy or audio sink.
- Existing `VdiscPlayer` QML module, `Main.qml`, and greybox asset remain intact.
- Controller decides legality; `vdisc-core` owns disc facts and playback position. No QML state machine.
- QML state properties have `READ, NOTIFY` but no `WRITE` setters. Expose rejections separately from machine errors.
- OPEN and disc requests publish intermediate mechanical state; only completion callbacks publish final state.
- Prefix every shell command with `rtk`. Do not include unrelated untracked files in commits; wait for explicit user commit request.

## Review Focus

1. OPEN during playback or HOLD: reject without changing lid; Task 1 tests both.
2. Completion callback before matching request: reject without changing state; Task 1 tests this.
3. Empty or invalid disc path: reject safely; failed validation leaves disc absent and projects machine error; Task 1 tests this.
4. `NaN`, infinite, negative, or >1 volume and negative scan target: reject without panic or state change; Tasks 1 and 3 test these inputs.
5. QML assignment to a projected property: property is read-only and cannot mutate Rust state; Task 3 tests QML behavior.

---

### Task 1: Rust command runtime

**Files:**
- Create: `crates/vdisc-qml-bridge/Cargo.toml`, `crates/vdisc-qml-bridge/src/lib.rs`, `crates/vdisc-qml-bridge/src/runtime.rs`, `crates/vdisc-qml-bridge/tests/runtime.rs`
- Modify: `Cargo.toml` workspace members

**Interfaces:**
- Produces `pub enum Command { Open, Close, Insert(PathBuf), Remove, Play, Pause, Stop, Previous, Next, ScanBegin(ScanDirection), ScanEnd, ScanStep(i64), MenuShort, MenuLong, SetHold(bool), SetVolume(f32), LidOpened, LidClosed, DiscInserted, DiscRemoved }`.
- Cargo library types are `staticlib` for Qt linking and `rlib` for Rust tests; Task 1 depends only on `vdisc-appliance`.
- Produces `pub struct ApplianceRuntime` with `new() -> Self`, `execute(&mut self, Command) -> Result<(), String>`, read-only `controller(&self) -> &De200Controller` and `backend(&self) -> &CorePlayerBridge` getters, and crate-visible controller/backend fields for projection. `Default` calls `new()`.
- `Insert` starts insertion, then calls `CorePlayerBridge::validate_inserting_disc`; `DiscInserted` calls `notify_disc_seated`; `DiscRemoved` calls `CorePlayerBridge::complete_disc_removal`.
- `MenuShort` calls `request_cycle_play_mode`; `MenuLong` calls `request_toggle_avls`. `ScanStep` converts nonnegative milliseconds to `PlaybackPosition` and calls `scan_seek`.

- [ ] **Step 1: Write failing Rust integration tests.** In `tests/runtime.rs`, assert initial `Closed/Absent/Stopped` and volume `0.5`; `Open` yields `Opening`, `LidOpened` yields `Open`; out-of-order `LidClosed` fails without mutation; HOLD rejects OPEN; valid fixture `../../tests/fixtures/vdisc/valid-v1.vdisc` completes insert/remove; invalid path reports error and returns disc to `Absent`; invalid volume and scan target fail without mutation. Add seated-disc tests for PLAY/PAUSE/STOP/PREVIOUS/NEXT, MENU, and scan begin/step/end.
- [ ] **Step 2: Run `rtk cargo test -p vdisc_qml_bridge --test runtime`; expect compilation or assertion failure for missing runtime API.**
- [ ] **Step 3: Implement `ApplianceRuntime::execute` with the exact command variants above.** Use existing controller/backend methods; map each error's `Display` to a nonempty rejection string. Reject empty insertion path before mechanical mutation. Use `Volume::new` for numeric validation. Do not create playback or disc state outside the existing two owners.
- [ ] **Step 4: Run `rtk cargo test -p vdisc_qml_bridge --test runtime`; expect all runtime tests to pass.**

### Task 2: Immutable state and LCD projection

**Files:**
- Create: `crates/vdisc-qml-bridge/src/projection.rs`, `crates/vdisc-qml-bridge/tests/projection.rs`
- Modify: `crates/vdisc-qml-bridge/src/lib.rs`

**Interfaces:**
- Consumes `ApplianceRuntime`, `De200Controller`, and `CorePlayerBridge` from Task 1.
- Produces `pub struct ApplianceSnapshot` with `lid_state`, `disc_state`, `transport_state`, `play_mode`, `hold_enabled`, `avls_enabled`, `volume`, `application_gain`, `machine_error`, `lcd_track_number`, `lcd_elapsed_ms`, `lcd_total_tracks`, `lcd_total_ms`, `lcd_play_mode`, `lcd_hold`, `lcd_avls`, `lcd_playback_status`, `lcd_message`.
- Produces `ApplianceRuntime::snapshot(&self) -> ApplianceSnapshot`. Enum and message values are stable PascalCase strings matching current Rust variants; absent string is `""`, absent numeric LCD field is `-1`; times are milliseconds.

- [ ] **Step 1: Write failing tests in `tests/projection.rs`.** Assert initial strings/empty LCD, `Open` changes lid projection, `SetVolume(0.8)` changes normalized volume and application gain, HOLD and AVLS project to LCD indicators, and valid fixture insertion projects track metadata. Check machine error after invalid disc bytes. Include large playback position conversion without panic.
- [ ] **Step 2: Run `rtk cargo test -p vdisc_qml_bridge --test projection`; expect missing snapshot API failure.**
- [ ] **Step 3: Implement `snapshot()` from controller getters, `CorePlayerBridge::lcd_facts()`, and `De200Controller::lcd_snapshot(..., None)`.** Convert optional numbers to `-1` and clamp unrepresentable `u64` milliseconds to `i64::MAX`. Do not store a second mutable copy of state.
- [ ] **Step 4: Run `rtk cargo test -p vdisc_qml_bridge --test projection`; expect all projection tests to pass.**

### Task 3: CXX-Qt QML object and build integration

**Files:**
- Create: `crates/vdisc-qml-bridge/build.rs`, `crates/vdisc-qml-bridge/src/qobject.rs`, `apps/vdisc-player/tests/tst_appliance_bridge.qml`
- Modify: `crates/vdisc-qml-bridge/Cargo.toml`, `crates/vdisc-qml-bridge/src/lib.rs`, `apps/vdisc-player/CMakeLists.txt`

**Interfaces:**
- Consumes `ApplianceRuntime::execute` and `snapshot` from Tasks 1–2.
- Produces QML module `VdiscAppliance 1.0` with creatable `ApplianceBridge`.
- Invokables return `bool`: `requestOpen`, `requestClose`, `requestInsert(QString)`, `requestRemove`, `requestPlay`, `requestPause`, `requestStop`, `requestPrevious`, `requestNext`, `requestScanBegin(bool forward)`, `requestScanEnd`, `requestScanStep(qint64 targetMs)`, `requestMenuShort`, `requestMenuLong`, `setHold(bool)`, `setVolume(float)`, `lidOpened`, `lidClosed`, `discInserted`, `discRemoved`.
- Read-only notified QML properties mirror all Task 2 snapshot fields in camelCase, plus `lastRejection`. Changed values notify after every request/completion; success clears `lastRejection`, failure sets it and refreshes machine error/LCD as needed.

- [ ] **Step 1: Write failing QML test.** Import `VdiscAppliance 1.0`; construct `ApplianceBridge`; assert `lidState === "Closed"`; call `requestOpen()` and assert `lidState === "Opening"`; call `lidOpened()` and assert `lidState === "Open"`; reject second `requestOpen()` without state change and with nonempty `lastRejection`; attempt to assign `lidState` from QML, catch any JavaScript error, and verify state remains `Open`; test `setVolume(NaN)` and `requestScanStep(-1)` rejection.
- [ ] **Step 2: Configure/build through `rtk cmake -S apps/vdisc-player -B /tmp/vdisc-phase03-obj04-build`; expect test to fail because QML bridge module is absent.** Use the linked `vdisc-appliance-tests` runner for QML tests once the static module is built.
- [ ] **Step 3: Add CXX-Qt 0.10 crate/build and CMake import.** Use `#[qobject]`, `#[qml_element]`, `#[qinvokable]`, and `#[qproperty(TYPE, NAME, READ, NOTIFY)]`; never generate QML setters. Build `QmlModule::new("VdiscAppliance")`; import/link through `cxx_qt_import_crate` and `cxx_qt_import_qml_module`. Raise CMake minimum to 3.24 and fetch version-matched CXX-Qt CMake integration if not installed. Keep Qt 6.8 minimum.
- [ ] **Step 4: Implement invokables as thin `Command` delegates and refresh only changed projection fields.** Keep `ApplianceRuntime` private to the Rust object. Guard `QString` path conversion and numeric conversion without panic.
- [ ] **Step 5: Run `rtk cargo test -p vdisc_qml_bridge`, `rtk env CARGO_PROFILE_DEV_DEBUG=0 cmake --build /tmp/vdisc-phase03-obj04-build -j 4`, and `rtk env QT_QPA_PLATFORM=offscreen /tmp/vdisc-phase03-obj04-build/vdisc-appliance-tests -input apps/vdisc-player/tests -import /tmp/vdisc-phase03-obj04-build/cxxqt/qml_modules`; expect all pass.**

### Task 4: Runtime instance and final gates

**Files:**
- Modify: `apps/vdisc-player/qml/Main.qml`
- Test: `apps/vdisc-player/tests/tst_appliance_bridge.qml`, `apps/vdisc-player/tests/tst_static_asset.qml`

**Interfaces:**
- Consumes `VdiscAppliance 1.0` and `ApplianceBridge` from Task 3.
- Produces `Main.qml` with exactly one private `ApplianceBridge` instance and a read-only alias for tests/future presentation wiring. No button actions or animation in this objective.

- [ ] **Step 1: Extend QML test to create `Main` and assert its bridge alias exists, starts `Closed`, and publishes `Opening` after `requestOpen()`; ensure existing greybox test still addresses same asset.**
- [ ] **Step 2: Run QML tests; expect missing `Main` bridge alias failure.**
- [ ] **Step 3: Import `VdiscAppliance 1.0` in `Main.qml`, create one `ApplianceBridge`, expose read-only alias; do not add QML machine state.**
- [ ] **Step 4: Run `rtk cargo test --workspace`, `rtk env CARGO_PROFILE_DEV_DEBUG=0 cmake --build /tmp/vdisc-phase03-obj04-build -j 4`, QML tests in offscreen mode through `vdisc-appliance-tests`, and `rtk /usr/lib/qt6/bin/qmllint -I /tmp/vdisc-phase03-obj04-build/cxxqt/qml_modules apps/vdisc-player/qml/Main.qml`; expect zero failures/errors.** Inspect `rtk git diff --check` and `rtk git status --short`; report any unavailable GUI gate separately. Do not commit until user asks.
