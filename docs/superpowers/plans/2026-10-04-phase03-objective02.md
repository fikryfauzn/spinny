# Phase 3 Objective 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Author and validate a Blender greybox hierarchy for the Phase 3 mechanical runtime.

**Architecture:** Create a separate Blender scene through Blender MCP, preserving the startup scene. Save a `.blend` checkpoint, export one GLB from that scene, and validate both Blender structure and GLB hierarchy. Geometry remains primitive and untextured.

**Tech Stack:** Blender 5.2.2 LTS, Blender MCP, Blender Python API, glTF 2.0.

**Spec:** `docs/superpowers/specs/2026-10-04-phase03-objective01-design.md` and `/home/dante/Projects/spinny/VDISC_PHASE_03_PLAN.md` Objective 2.

## Global Constraints

- Authoring dimensions use meters; Blender X right, Y rear, Z up.
- Overall envelope is 0.131 × 0.148 × 0.028 m.
- Preserve exact names, parents, rest transforms, lid hinge, and disc spindle pivots from the contract.
- Do not alter the original default scene or Phase 2 files.
- Do not commit or push; user owns commits under Phase 3 workflow.

## Review Focus

- Duplicate names with `.001` suffix must fail validation.
- A parent changed or a moving pivot shifted must fail validation.
- Non-unit object scale must fail validation.
- GLB export must contain only the greybox scene and preserve names and hierarchy.
- Lid, disc, buttons, and body must not intersect in their closed rest pose beyond intended contact.

---

### Task 1: Primitive scene and hierarchy

**Files:** `assets/blender/phase03-greybox-02.blend`

**Interfaces:** Produces the exact object names and parent hierarchy in the Objective 1 contract.

- [x] Inspect the current Blender scene, file path, objects, transforms, and units.
- [x] Create a separate `VDISC_PHASE03_GREYBOX` scene with `PLAYER_ROOT` and a primitive housing frame.
- [x] Inspect the resulting root and `BODY_TEST` before creating moving parts.
- [x] Add `LID_ROOT`/`LID_TEST`, inspect pivot and closed placement.
- [x] Add `DISC_ROOT`/`DISC_TEST`, inspect spindle center and clearance.
- [x] Add PLAY, PAUSE, STOP, OPEN caps and `LCD_TEST`, inspect names and alignment.
- [x] Save `phase03-greybox-02.blend` without altering the original scene.

### Task 2: Export and verification

**Files:** `scripts/validate_phase03_greybox.py`, `scripts/export_phase03_greybox.py`, `scripts/validate_phase03_glb.py`, `assets/runtime/phase03-greybox.glb`

**Interfaces:** Produces a GLB suitable for Objective 3 import and a repeatable validation/export command.

- [x] Add Blender background validator for names, parents, transforms, pivot coordinates, dimensions, and source scene.
- [x] Run validator on the saved `.blend`; 12 objects and closed bounds passed.
- [x] Add an export script limited to the greybox scene with GLB/Y-up settings.
- [x] Export, inspect GLB JSON nodes, and check 12 names, parents, and transforms.
- [x] Reject hidden Blender parent-inverse/delta/quaternion transforms and glTF `matrix` transforms; negative regression scripts confirm rejection.
- [x] Derive closed-lid and wall clearance checks from saved world geometry.
- [x] Repeat export; SHA-256 remained `76da9e8823900c077a0ce0a9af5d92c59baa0074831496c8b2e0eb8a41a979e2`.
- [x] Capture a Blender viewport image with the lid open; inspect visible disc and control clearance. Restore closed rest pose.
- [x] Review `git status` and retain existing Phase 2 logs untouched.
