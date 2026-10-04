# Phase 3 Objective 1: Runtime and Asset Contract

Status: locked for the Phase 3 greybox. This contract describes test geometry, not measured D-E200 details.

## Goal

Launch a small Qt Quick 3D application on Arch/Wayland and render one primitive at a known physical scale. Fix the Rust/QML ownership and Blender import rules before building the greybox.

## Runtime boundary

- Qt 6.8 is the minimum; Qt 6.11.2 is the verified development version. CMake builds the Qt application in `apps/vdisc-player/` without changing the Cargo workspace.
- `vdisc-core` owns disc data and playback. `vdisc-appliance` owns legal actions and machine state. QML requests actions and displays projected state; it never decides legality.
- Objective 4 will expose a Rust-owned `QObject` to QML through CXX-Qt. The object will wrap the appliance controller and the existing core state bridge, offer invokable command requests and read-only state properties, and accept explicit lid/disc animation completion callbacks. The real audio path joins in Objective 9. Objective 1 does not add a placeholder state machine.
- QML owns the Qt Quick 3D scene, hit testing, and presentation animation. Blender owns meshes, hierarchy, and pivots. Completion signals return to Rust only after animations finish.

## Coordinate and size contract

- Blender authoring: metric, 1 Blender unit = 1 meter. `PLAYER_ROOT` is at the center of the player's underside. Blender X points right, Y points to the rear hinge, and Z points up.
- Export: glTF 2.0 `.glb` with Blender's Y-up conversion. Qt scene X points right, Y points up, and Z points toward the front. Thus Blender `(x, y, z)` maps to Qt `(x, z, -y)` before unit conversion.
- Qt Quick 3D scene: 1 unit = 1 centimeter. glTF meters become scene centimeters through one explicit Balsam import scale of 100. No QML root-scale correction is permitted for imported assets.
- Greybox envelope: 131 mm wide, 148 mm deep, 28 mm high. In Blender: `(0.131, 0.148, 0.028)` m. In Qt: `(13.1, 2.8, 14.8)` scene units for X/Y/Z extent.
- Coordinates for the hinge and spindle below are greybox assumptions. Phase 4 physical reference work may revise their measured placement without changing the ownership rule.

## Initial asset hierarchy

```text
PLAYER_ROOT (Empty, origin 0,0,0)
├── BODY_TEST (Mesh)
├── LID_ROOT (Empty; hinge pivot, rear center)
│   └── LID_TEST (Mesh)
├── DISC_ROOT (Empty; spindle center)
│   └── DISC_TEST (Mesh)
├── SPINDLE_TEST (Mesh; fixed hub)
├── BTN_PLAY_TEST (Mesh)
├── BTN_PAUSE_TEST (Mesh)
├── BTN_STOP_TEST (Mesh)
├── BTN_OPEN_TEST (Mesh)
└── LCD_TEST (Mesh)
```

`LID_ROOT` starts at Blender `(0, +0.074, +0.028)` m and opens around local negative X. `DISC_ROOT` starts at `(0, 0, +0.014)` m and spins about local Z. Button origins sit at cap centers and travel along local negative Z. These are prototype placements and travel directions, not claims about exact Sony measurements. Moving-object scales must be applied before export; object names and parent relationships are API contracts.

Objective 2 fixes primitive placement for the mechanical proof. `BODY_TEST` is an 8 mm base with 3 mm side walls and a front control strip. `LID_TEST` covers the disc from Y = -65 mm to the rear hinge at +74 mm. The disc has a 60 mm outer radius and a 7.5 mm hub hole; `SPINDLE_TEST` has a 6.5 mm radius. These dimensions are greybox clearance choices, not physical reference measurements.

Future controls use `BTN_PREVIOUS_TEST`, `BTN_NEXT_TEST`, `BTN_MENU_TEST`, `HOLD_SWITCH_TEST`, and `VOLUME_CONTROL_TEST` as children of `PLAYER_ROOT`. They enter only when their objective needs them.

## Asset and build paths

- Blender source checkpoint: `assets/blender/phase03-greybox-02.blend`. The original Blender startup scene remains inside the file; `VDISC_PHASE03_GREYBOX` is the active asset scene.
- Deterministic Blender export: `assets/runtime/phase03-greybox.glb`.
- Balsam output: `apps/vdisc-player/generated/phase03-greybox/`. This is generated from the `.glb`, never hand-edited. Objective 2 locks the export script and validator; Objective 3 integrates the output into the QML module.
- Objective 1 uses a Qt built-in `#Cube` as the known primitive. Its dimensions match the greybox envelope. This proves the scene contract without pretending that asset ingestion has already passed.
- Objective 2 validation/export commands are:

```bash
blender --background assets/blender/phase03-greybox-02.blend --python-exit-code 1 --python scripts/validate_phase03_greybox.py
blender --background assets/blender/phase03-greybox-02.blend --python-exit-code 1 --python scripts/export_phase03_greybox.py
python3 scripts/validate_phase03_glb.py assets/runtime/phase03-greybox.glb
blender --background assets/blender/phase03-greybox-02.blend --python-exit-code 1 --python scripts/test_phase03_greybox_validator.py
python3 scripts/test_phase03_glb_validator.py
```

## Objective 1 gate

Configure and build with CMake, lint QML with the installed Qt tool, launch on Wayland, and visually confirm the primitive. Record any unavailable gate as unavailable. No actual audio, controller bridge, lid mechanics, or final model is claimed in this objective.

```bash
cmake -S apps/vdisc-player -B /tmp/vdisc-phase03-obj01-build
cmake --build /tmp/vdisc-phase03-obj01-build -j 4
/usr/lib/qt6/bin/qmllint apps/vdisc-player/qml/Main.qml
QT_QPA_PLATFORM=wayland /tmp/vdisc-phase03-obj01-build/vdisc-player
```

The current Arch package installs `balsam` at `/usr/lib/qt6/bin/balsam`. For Objective 3, import the Blender `.glb` with an explicit 100× global scale, then validate the resulting bounds and hierarchy. Do not infer that the asset pipeline passed from the Objective 1 built-in cube.

## Documentation basis

- [Qt Quick 3D introduction](https://doc.qt.io/qt-6/qtquick3d-intro-example.html)
- [Qt Quick CMake application](https://doc.qt.io/qt-6/cmake-build-qml-application.html)
- [Qt Balsam asset import](https://doc.qt.io/qt-6/qtquick3d-tool-balsam.html)
- [CXX-Qt book](https://kdab.github.io/cxx-qt/book/)
