"""Extend the saved Objective 7 source without changing its original geometry.

Execute inside Blender with the source checkpoint open. The destination must
not exist. Source validation and snapshots protect the existing fourteen objects.
"""
from pathlib import Path
import sys
import bpy
from mathutils import Matrix

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
from validate_phase03_greybox import LEGACY_PARENTS, PARENTS, LOCATIONS, DIMENSIONS, SCENE_NAME, validate_scene

repo = Path(__file__).resolve().parents[1]
baseline = repo / "assets/blender/phase03-controls-proof.blend"
output = repo / "assets/blender/phase03-complete-controls-proof.blend"
if Path(bpy.data.filepath).resolve() != baseline or output.exists():
    raise RuntimeError("Open original controls checkpoint; destination must not exist")
scene = bpy.context.scene
if scene.name != SCENE_NAME or validate_scene(profile="legacy")[0]:
    raise RuntimeError("Unexpected legacy source")

def snapshot(obj):
    return (obj.type, obj.parent.name if obj.parent else None,
            tuple(tuple(row) for row in obj.matrix_local),
            tuple(tuple(v.co) for v in obj.data.vertices) if obj.type == "MESH" else ())

before = {name: snapshot(scene.objects[name]) for name in LEGACY_PARENTS}
root = scene.objects["PLAYER_ROOT"]
collection = scene.objects["BTN_PLAY_TEST"].users_collection[0]
material = bpy.data.materials["GREY_BUTTON"]
corners = ((-1,-1,-1), (1,-1,-1), (1,1,-1), (-1,1,-1),
           (-1,-1,1), (1,-1,1), (1,1,1), (-1,1,1))
for name in sorted(set(PARENTS) - set(LEGACY_PARENTS)):
    dimensions = DIMENSIONS[name]
    mesh = bpy.data.meshes.new(name + "_MESH")
    mesh.from_pydata([tuple(sign * size / 2 for sign, size in zip(corner, dimensions))
                     for corner in corners], [],
                    [(0,3,2,1), (4,5,6,7), (0,1,5,4), (1,2,6,5), (2,3,7,6), (3,0,4,7)])
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    obj.parent = root
    obj.matrix_parent_inverse = Matrix.Identity(4)
    obj.location = LOCATIONS[name]
    obj.rotation_mode = "XYZ"
    obj.data.materials.append(material)
bpy.context.view_layer.update()
if any(snapshot(scene.objects[name]) != saved for name, saved in before.items()):
    raise RuntimeError("Existing geometry changed")
errors, summary = validate_scene()
if errors:
    raise RuntimeError("Invalid complete checkpoint: " + "; ".join(errors))
status = bpy.ops.wm.save_as_mainfile(filepath=str(output), check_existing=False)
if "FINISHED" not in status:
    raise RuntimeError("Checkpoint save failed")
print({"saved": str(output), **summary})
