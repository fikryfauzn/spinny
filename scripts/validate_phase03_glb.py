"""Check names and parents in the exported Phase 3 GLB."""

import json
import math
from pathlib import Path
import struct
import sys


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
TRANSLATIONS = {
    "LID_ROOT": (0, 0.028, -0.074),
    "DISC_ROOT": (0, 0.014, 0),
    "BTN_PLAY_TEST": (-0.048, 0.0235, 0.069),
    "BTN_PAUSE_TEST": (-0.033, 0.0235, 0.069),
    "BTN_STOP_TEST": (-0.018, 0.0235, 0.069),
    "BTN_OPEN_TEST": (-0.003, 0.0235, 0.069),
    "BTN_PREVIOUS_TEST": (-0.033, 0.013, 0.073),
    "BTN_NEXT_TEST": (-0.015, 0.013, 0.073),
    "LCD_TEST": (0.033, 0.0221, 0.069),
}


PARENTS.update({
    "BTN_MENU_TEST": "PLAYER_ROOT",
    "HOLD_RAIL_TEST": "PLAYER_ROOT",
    "HOLD_KNOB_TEST": "PLAYER_ROOT",
    "VOLUME_RAIL_TEST": "PLAYER_ROOT",
    "VOLUME_KNOB_TEST": "PLAYER_ROOT",
})
TRANSLATIONS.update({
    "BTN_MENU_TEST": (0.003, 0.013, 0.073),
    "HOLD_RAIL_TEST": (0.02, 0.013, 0.073),
    "HOLD_KNOB_TEST": (0.017, 0.013, 0.0735),
    "VOLUME_RAIL_TEST": (0.043, 0.013, 0.073),
    "VOLUME_KNOB_TEST": (0.043, 0.013, 0.0735),
})

def close(actual, expected):
    return all(
        math.isclose(float(a), float(b), rel_tol=0, abs_tol=1e-5)
        for a, b in zip(actual, expected, strict=True)
    )


def read_glb_json(path):
    data = path.read_bytes()
    if len(data) < 20:
        raise ValueError("GLB is too short")
    magic, version, length = struct.unpack_from("<4sII", data)
    if magic != b"glTF" or version != 2 or length != len(data):
        raise ValueError("Invalid GLB header")
    offset = 12
    while offset + 8 <= len(data):
        size, kind = struct.unpack_from("<I4s", data, offset)
        offset += 8
        chunk = data[offset : offset + size]
        offset += size
        if kind == b"JSON":
            return json.loads(chunk)
    raise ValueError("GLB has no JSON chunk")


def validate(path):
    document = read_glb_json(path)
    nodes = document.get("nodes", [])
    by_name = {}
    for index, node in enumerate(nodes):
        name = node.get("name")
        if name in by_name:
            raise ValueError(f"Duplicate GLB node: {name}")
        by_name[name] = index

    missing = set(PARENTS) - set(by_name)
    if missing:
        raise ValueError(f"Missing GLB nodes: {sorted(missing)}")
    extra = set(by_name) - set(PARENTS)
    if extra:
        raise ValueError(f"Unexpected GLB nodes: {sorted(map(str, extra))}")

    parent_of = {}
    for index, node in enumerate(nodes):
        for child in node.get("children", []):
            parent_of[child] = index
    for name, expected_parent in PARENTS.items():
        node = nodes[by_name[name]]
        if "matrix" in node:
            raise ValueError(f"{name}: matrix transform is not permitted")
        parent_index = parent_of.get(by_name[name])
        parent_name = nodes[parent_index].get("name") if parent_index is not None else None
        if parent_name != expected_parent:
            raise ValueError(f"{name}: GLB parent {parent_name!r}, expected {expected_parent!r}")
        has_mesh = "mesh" in node
        if has_mesh != (name not in {"PLAYER_ROOT", "LID_ROOT", "DISC_ROOT"}):
            raise ValueError(f"{name}: wrong GLB mesh status")
        if not close(node.get("translation", (0, 0, 0)), TRANSLATIONS.get(name, (0, 0, 0))):
            raise ValueError(f"{name}: wrong Y-up translation")
        if not close(node.get("rotation", (0, 0, 0, 1)), (0, 0, 0, 1)):
            raise ValueError(f"{name}: unexpected rotation")
        if not close(node.get("scale", (1, 1, 1)), (1, 1, 1)):
            raise ValueError(f"{name}: unapplied scale")

    scene_index = document.get("scene", 0)
    scene = document["scenes"][scene_index]
    if by_name["PLAYER_ROOT"] not in scene.get("nodes", []):
        raise ValueError("PLAYER_ROOT is not a scene root")
    return {"status": "ok", "nodes": len(nodes), "named_contract_nodes": len(PARENTS)}


if __name__ == "__main__":
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("assets/runtime/phase03-greybox.glb")
    print(json.dumps(validate(path), sort_keys=True))
