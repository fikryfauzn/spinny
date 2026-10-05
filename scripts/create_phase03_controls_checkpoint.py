"""Add the Objective 7 controls to the saved baseline without overwriting it.

Run: blender --background assets/blender/phase03-greybox-02.blend \
    --python-exit-code 1 --python scripts/create_phase03_controls_checkpoint.py
"""

from pathlib import Path
import sys

import bpy
from mathutils import Matrix

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
from validate_phase03_greybox import PARENTS, SCENE_NAME, validate_scene

repo = Path(__file__).resolve().parents[1]
baseline = repo / "assets/blender/phase03-greybox-02.blend"
output = repo / "assets/blender/phase03-controls-proof.blend"
if Path(bpy.data.filepath).resolve() != baseline or output.exists():
    raise RuntimeError("Open the original checkpoint; destination must not already exist")
scene = bpy.context.scene
new_names = {"BTN_PREVIOUS_TEST", "BTN_NEXT_TEST"}
if scene.name != SCENE_NAME or {obj.name for obj in scene.objects} != set(PARENTS) - new_names:
    raise RuntimeError("Unexpected baseline scene/object contract")


def snapshot(obj):
    return (obj.type, obj.parent.name if obj.parent else None,
            tuple(tuple(row) for row in obj.matrix_local),
            tuple(tuple(v.co) for v in obj.data.vertices) if obj.type == "MESH" else ())


before = {obj.name: snapshot(obj) for obj in scene.objects}
root = scene.objects["PLAYER_ROOT"]
collection = scene.objects["BTN_PLAY_TEST"].users_collection[0]
material = bpy.data.materials["GREY_BUTTON"]
for name, x in (("BTN_PREVIOUS_TEST", -0.033), ("BTN_NEXT_TEST", -0.015)):
    mesh = bpy.data.meshes.new(name + "_MESH")
    vertices = [(sx * 0.006, sy * 0.0015, sz * 0.003)
                for sx, sy, sz in ((-1,-1,-1), (1,-1,-1), (1,1,-1), (-1,1,-1),
                                  (-1,-1,1), (1,-1,1), (1,1,1), (-1,1,1))]
    mesh.from_pydata(vertices, [], [(0,3,2,1), (4,5,6,7), (0,1,5,4),
                                   (1,2,6,5), (2,3,7,6), (3,0,4,7)])
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    obj.parent = root
    obj.matrix_parent_inverse = Matrix.Identity(4)
    obj.location = (x, -0.073, 0.013)
    obj.rotation_mode = "XYZ"
    obj.data.materials.append(material)

bpy.context.view_layer.update()
if any(snapshot(scene.objects[name]) != saved for name, saved in before.items()):
    raise RuntimeError("An existing object changed")
errors, summary = validate_scene()
if errors:
    raise RuntimeError("Invalid controls checkpoint: " + "; ".join(errors))
status = bpy.ops.wm.save_as_mainfile(filepath=str(output), check_existing=False)
if "FINISHED" not in status:
    raise RuntimeError(f"Saving checkpoint failed: {status}")
print({"saved": str(output), **summary})
