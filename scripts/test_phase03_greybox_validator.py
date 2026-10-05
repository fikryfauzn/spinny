"""Negative regression checks for the Blender source validator.

Run with the same .blend and Blender flags as validate_phase03_greybox.py.
This mutates only the in-memory scene; it does not save the file.
"""

import sys
from pathlib import Path

import bpy
from mathutils import Matrix

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
from validate_phase03_greybox import validate_scene  # noqa: E402


def must_fail(label, fragment):
    bpy.context.view_layer.update()
    errors, _ = validate_scene()
    assert any(fragment in error for error in errors), (label, errors)


assert not validate_scene()[0]
assert len(bpy.context.scene.objects) == 14, "Physical PREVIOUS/NEXT nodes are required"

for name in ("BTN_PREVIOUS_TEST", "BTN_NEXT_TEST"):
    obj = bpy.data.objects[name]
    obj.name = "WRONG_BUTTON"
    must_fail("missing control", "Missing objects")
    obj.name = name
    parent = obj.parent
    obj.parent = bpy.data.objects["LID_ROOT"]
    must_fail("wrong control parent", "parent")
    obj.parent = parent
    obj.scale.x = 2
    must_fail("scaled control", "unapplied scale")
    obj.scale.x = 1
    obj.location.x += 0.002
    must_fail("shifted control", "location")
    obj.location.x -= 0.002
    obj.data.vertices[0].co.x += 0.02
    must_fail("wrong control dimensions", "dimensions")
    obj.data.vertices[0].co.x -= 0.02

disc = bpy.data.objects["DISC_ROOT"]
disc.delta_location.x = 0.002
must_fail("shifted spindle", "effective world transform")
disc.delta_location.x = 0

disc.matrix_parent_inverse = Matrix.Translation((0.002, 0, 0))
must_fail("parent inverse", "nonidentity parent inverse")
disc.matrix_parent_inverse = Matrix.Identity(4)

disc.rotation_mode = "QUATERNION"
must_fail("quaternion mode", "unsupported rotation mode")
disc.rotation_mode = "XYZ"

bpy.context.view_layer.update()
assert not validate_scene()[0]
print("Source validator negative tests: ok")
