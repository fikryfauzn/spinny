"""Validate and export the Phase 3 greybox as glTF 2.0 GLB.

Run: blender --background assets/blender/phase03-complete-controls-proof.blend \
    --python-exit-code 1 --python scripts/export_phase03_greybox.py
"""

from pathlib import Path
import sys

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
from validate_phase03_greybox import SCENE_NAME, validate_scene  # noqa: E402


errors, _ = validate_scene()
if errors:
    raise RuntimeError("Invalid greybox source: " + "; ".join(errors))
if bpy.context.scene.name != SCENE_NAME:
    raise RuntimeError(f"Active scene is {bpy.context.scene.name}, expected {SCENE_NAME}")

output = Path(__file__).resolve().parents[1] / "assets/runtime/phase03-greybox.glb"
output.parent.mkdir(parents=True, exist_ok=True)
status = bpy.ops.export_scene.gltf(
    filepath=str(output),
    check_existing=False,
    export_format="GLB",
    use_active_scene=True,
    export_yup=True,
    export_cameras=False,
    export_lights=False,
)
if "FINISHED" not in status:
    raise RuntimeError(f"glTF export failed: {status}")
print(f"Exported {output} ({output.stat().st_size} bytes)")
