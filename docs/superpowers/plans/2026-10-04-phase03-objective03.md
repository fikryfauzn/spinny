# Phase 3 Objective 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the Blender greybox in the Qt Quick 3D application without adding behavior.

**Architecture:** Balsam converts the validated GLB into a generated QML scene and meshes under `apps/vdisc-player/generated/phase03-greybox/`. Qt packages those files into the existing QML module. The runtime instantiates that scene instead of the Objective 1 cube; validation checks names, hierarchy, positions, scale, and rendering.

**Tech Stack:** Qt 6.11.2, Qt Quick 3D, Balsam, CMake, Python 3.

**Spec:** `docs/superpowers/specs/2026-10-04-phase03-objective01-design.md` and `/home/dante/Projects/spinny/VDISC_PHASE_03_PLAN.md` Objective 3.

## Global Constraints

- Preserve the exact 12 named contract objects and their parent relationships.
- Qt uses centimeters: Balsam import applies the one explicit 100× scale. No hand-edited generated QML and no additional QML scale or rotation correction.
- The source GLB remains `assets/runtime/phase03-greybox.glb`; generated output remains under `apps/vdisc-player/generated/phase03-greybox/`.
- Leave Rust state, animation, interaction, audio, and Phase 2 files untouched.
- Do not commit or push; the user controls commits for Phase 3.
- Arch `assimp` package is required for Qt's Balsam GLB importer plugin.

## Review Focus

- Missing `assimp` must fail clearly, not produce a silently empty scene.
- A Balsam regeneration that drops or reparents a named node must fail validation.
- A second scale/rotation in `Main.qml` must fail validation.
- A generated mesh missing from resources must fail validation or build.
- Runtime must show the housing, lid, disc, controls, and LCD rather than only compile.

---

### Task 1: Generate and validate Qt asset

**Files:** `apps/vdisc-player/generated/phase03-greybox/`, `scripts/validate_phase03_qt_asset.py`

**Interfaces:** Produces `Phase03_greybox.qml` with named QML nodes and nine referenced `.mesh` files.

- [x] Write validator for exact object names, parents, Y-up local positions, one root scale of `(100, 100, 100)`, no other scale/rotation, and existing mesh references.
- [x] Run validator before import; failed because generated asset did not exist.
- [x] Run Balsam with `--globalScale --globalScaleValue 100 --outputPath apps/vdisc-player/generated/phase03-greybox assets/runtime/phase03-greybox.glb`.
- [x] Run validator; 12 named nodes, nine meshes, and contract positions passed. A separate import matched byte-for-byte.

### Task 2: Package and render Qt asset

**Files:** `apps/vdisc-player/CMakeLists.txt`, `apps/vdisc-player/qml/Main.qml`, `apps/vdisc-player/tests/tst_static_asset.qml`, `apps/vdisc-player/tests/OpenLidPreview.qml`

**Interfaces:** `Main.qml` instantiates `Phase03_greybox` from the generated relative import; CMake packages QML and meshes.

- [x] Write Qt Quick Test that creates `Main`, addresses the generated object tree by name, and checks world pivot positions in centimeters.
- [x] Run Qt Quick Test before edits; failed because `Main.greybox` was absent.
- [x] Add generated QML/mesh resources to `qt_add_qml_module`; replace cube with `Phase03_greybox` and keep existing camera and lighting.
- [x] Run Qt Quick Test on Wayland; 3 passed, 0 failed, no warnings. Identity-orientation guard failed under a deliberate 1° rotation and passed after its removal.
- [x] Configure and build in `/tmp/vdisc-phase03-obj03-build`; lint authored, generated, and preview QML.
- [x] Launch on Wayland and inspect closed-lid app plus test-only open-lid preview. Both rendered; no runtime warnings. Screenshots: `/tmp/vdisc-phase03-obj03-screenshot.png`, `/tmp/vdisc-phase03-obj03-open-lid.png`.
- [x] Run Objective 2 source/GLB validators to ensure asset contract remains intact, then inspect `git status` for unrelated changes.
