"""Validate the Phase 3 Blender source before export.

Run: blender --background assets/blender/phase03-controls-proof.blend \
    --python-exit-code 1 --python scripts/validate_phase03_greybox.py
"""

import json
import math
import sys

import bpy
from mathutils import Matrix, Vector


SCENE_NAME = "VDISC_PHASE03_GREYBOX"
PARENTS = {
    "PLAYER_ROOT": None,
    "BODY_TEST": "PLAYER_ROOT",
    "LID_ROOT": "PLAYER_ROOT",
    "LID_TEST": "LID_ROOT",
    "DISC_ROOT": "PLAYER_ROOT",
    "DISC_TEST": "DISC_ROOT",
    "SPINDLE_TEST": "PLAYER_ROOT",
    "BTN_PLAY_TEST": "PLAYER_ROOT",
    "BTN_PAUSE_TEST": "PLAYER_ROOT",
    "BTN_STOP_TEST": "PLAYER_ROOT",
    "BTN_OPEN_TEST": "PLAYER_ROOT",
    "BTN_PREVIOUS_TEST": "PLAYER_ROOT",
    "BTN_NEXT_TEST": "PLAYER_ROOT",
    "LCD_TEST": "PLAYER_ROOT",
}
EMPTY_NAMES = {"PLAYER_ROOT", "LID_ROOT", "DISC_ROOT"}
LOCATIONS = {
    "PLAYER_ROOT": (0, 0, 0),
    "BODY_TEST": (0, 0, 0),
    "LID_ROOT": (0, 0.074, 0.028),
    "LID_TEST": (0, 0, 0),
    "DISC_ROOT": (0, 0, 0.014),
    "DISC_TEST": (0, 0, 0),
    "SPINDLE_TEST": (0, 0, 0),
    "BTN_PLAY_TEST": (-0.048, -0.069, 0.0235),
    "BTN_PAUSE_TEST": (-0.033, -0.069, 0.0235),
    "BTN_STOP_TEST": (-0.018, -0.069, 0.0235),
    "BTN_OPEN_TEST": (-0.003, -0.069, 0.0235),
    "BTN_PREVIOUS_TEST": (-0.033, -0.073, 0.013),
    "BTN_NEXT_TEST": (-0.015, -0.073, 0.013),
    "LCD_TEST": (0.033, -0.069, 0.0221),
}
DIMENSIONS = {
    "BODY_TEST": (0.131, 0.148, 0.022),
    "LID_TEST": (0.131, 0.139, 0.006),
    "DISC_TEST": (0.120, 0.120, 0.0012),
    "SPINDLE_TEST": (0.013, 0.013, 0.009),
    "BTN_PLAY_TEST": (0.010, 0.006, 0.003),
    "BTN_PAUSE_TEST": (0.010, 0.006, 0.003),
    "BTN_STOP_TEST": (0.010, 0.006, 0.003),
    "BTN_OPEN_TEST": (0.010, 0.006, 0.003),
    "BTN_PREVIOUS_TEST": (0.012, 0.003, 0.006),
    "BTN_NEXT_TEST": (0.012, 0.003, 0.006),
    "LCD_TEST": (0.045, 0.006, 0.0002),
}
TOLERANCE = 1e-5


def close(actual, expected):
    return all(
        math.isclose(float(a), float(b), rel_tol=0, abs_tol=TOLERANCE)
        for a, b in zip(actual, expected, strict=True)
    )


def close_matrix(actual, expected):
    return all(close(actual[row], expected[row]) for row in range(4))


def world_vertices(obj):
    return [obj.matrix_world @ vertex.co for vertex in obj.data.vertices]


def bounds(objects):
    points = [
        obj.matrix_world @ Vector(corner)
        for obj in objects
        for corner in obj.bound_box
    ]
    return (
        tuple(min(point[axis] for point in points) for axis in range(3)),
        tuple(max(point[axis] for point in points) for axis in range(3)),
    )


def validate_scene():
    errors = []
    scene = bpy.data.scenes.get(SCENE_NAME)
    if scene is None:
        return [f"Missing scene {SCENE_NAME}"], {}

    if scene.unit_settings.system != "METRIC" or not math.isclose(
        scene.unit_settings.scale_length, 1.0, abs_tol=1e-8
    ):
        errors.append("Scene must use metric units at 1 Blender unit per meter")

    actual_names = {obj.name for obj in scene.objects}
    missing = set(PARENTS) - actual_names
    extra = actual_names - set(PARENTS)
    if missing:
        errors.append(f"Missing objects: {sorted(missing)}")
    if extra:
        errors.append(f"Unexpected objects: {sorted(extra)}")

    expected_world = {}
    for name, expected_parent in PARENTS.items():
        obj = scene.objects.get(name)
        if obj is None:
            continue
        parent = obj.parent.name if obj.parent else None
        if parent != expected_parent:
            errors.append(f"{name}: parent {parent!r}, expected {expected_parent!r}")
        expected_type = "EMPTY" if name in EMPTY_NAMES else "MESH"
        if obj.type != expected_type:
            errors.append(f"{name}: type {obj.type}, expected {expected_type}")
        if not close(obj.location, LOCATIONS[name]):
            errors.append(f"{name}: location {tuple(obj.location)}")
        if not close(obj.rotation_euler, (0, 0, 0)):
            errors.append(f"{name}: rest rotation {tuple(obj.rotation_euler)}")
        if not close(obj.scale, (1, 1, 1)):
            errors.append(f"{name}: unapplied scale {tuple(obj.scale)}")
        if obj.rotation_mode != "XYZ":
            errors.append(f"{name}: unsupported rotation mode {obj.rotation_mode}")
        if not close(obj.delta_location, (0, 0, 0)) or not close(
            obj.delta_rotation_euler, (0, 0, 0)
        ) or not close(obj.delta_scale, (1, 1, 1)):
            errors.append(f"{name}: unexpected delta transform")
        if not close_matrix(obj.matrix_parent_inverse, Matrix.Identity(4)):
            errors.append(f"{name}: nonidentity parent inverse")
        expected_local = Matrix.Translation(Vector(LOCATIONS[name]))
        expected_world[name] = (
            expected_world[expected_parent] @ expected_local
            if expected_parent in expected_world
            else expected_local
        )
        if not close_matrix(obj.matrix_local, expected_local):
            errors.append(f"{name}: effective local transform differs from contract")
        if not close_matrix(obj.matrix_world, expected_world[name]):
            errors.append(f"{name}: effective world transform differs from contract")
        if name in DIMENSIONS and not close(obj.dimensions, DIMENSIONS[name]):
            errors.append(f"{name}: dimensions {tuple(obj.dimensions)}")
        if obj.constraints or (obj.type == "MESH" and obj.modifiers):
            errors.append(f"{name}: unresolved modifier or constraint")

    if not errors:
        mesh_objects = [obj for obj in scene.objects if obj.type == "MESH"]
        minimum, maximum = bounds(mesh_objects)
        if not close(minimum, (-0.0655, -0.0745, 0)) or not close(
            maximum, (0.0655, 0.074, 0.028)
        ):
            errors.append(f"Wrong closed envelope: {minimum} to {maximum}")

        disc = scene.objects["DISC_TEST"]
        spindle = scene.objects["SPINDLE_TEST"]
        body = scene.objects["BODY_TEST"]
        lid = scene.objects["LID_TEST"]
        disc_points = world_vertices(disc)
        spindle_points = world_vertices(spindle)
        body_points = world_vertices(body)
        disc_radii = [math.hypot(v.x, v.y) for v in disc_points]
        spindle_radii = [math.hypot(v.x, v.y) for v in spindle_points]
        if not math.isclose(min(disc_radii), 0.0075, abs_tol=TOLERANCE):
            errors.append("Disc hub-hole radius must be 7.5 mm")
        if not math.isclose(max(disc_radii), 0.060, abs_tol=TOLERANCE):
            errors.append("Disc outer radius must be 60 mm")
        if max(spindle_radii) >= min(disc_radii):
            errors.append("Spindle intersects disc hub hole")
        wall_points = [point for point in body_points if point.z > 0.008 + TOLERANCE]
        side_wall = min(abs(point.x) for point in wall_points if abs(point.x) > 0.060)
        end_wall = min(abs(point.y) for point in wall_points if abs(point.y) > 0.060)
        if max(disc_radii) >= side_wall:
            errors.append("Disc intersects housing side walls")
        if max(disc_radii) >= end_wall:
            errors.append("Disc intersects housing front or rear wall")
        lid_bottom = min(point.z for point in world_vertices(lid))
        if max(point.z for point in disc_points) >= lid_bottom:
            errors.append("Disc intersects closed lid")
        lid_min, lid_max = bounds([lid])
        for name in (*[n for n in PARENTS if n.startswith("BTN_")], "LCD_TEST"):
            control_min, control_max = bounds([scene.objects[name]])
            overlap_x = control_min[0] < lid_max[0] and control_max[0] > lid_min[0]
            overlap_y = control_min[1] < lid_max[1] and control_max[1] > lid_min[1]
            overlap_z = control_min[2] < lid_max[2] and control_max[2] > lid_min[2]
            if overlap_x and overlap_y and overlap_z:
                errors.append(f"{name} intersects closed lid")

        summary = {
            "scene": scene.name,
            "objects": len(scene.objects),
            "closed_bounds_m": [list(minimum), list(maximum)],
            "hinge_m": list(scene.objects["LID_ROOT"].matrix_world.translation),
            "spindle_m": list(scene.objects["DISC_ROOT"].matrix_world.translation),
            "disc_hole_clearance_m": min(disc_radii) - max(spindle_radii),
        }
    else:
        summary = {}
    return errors, summary


if __name__ == "__main__":
    errors, summary = validate_scene()
    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
    print(json.dumps({"status": "ok", **summary}, sort_keys=True))
