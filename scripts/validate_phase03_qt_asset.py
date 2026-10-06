"""Validate the Balsam-generated Objective 3 QML scene and mesh references."""

import json
import math
from pathlib import Path
import re
import sys


ASSET = Path("apps/vdisc-player/generated/phase03-greybox/Phase03_greybox.qml")
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
POSITIONS = {
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
VECTOR = re.compile(r"Qt\.vector3d\(([^)]+)\)")
PARENTS.update({
    "BTN_MENU_TEST": "PLAYER_ROOT",
    "HOLD_RAIL_TEST": "PLAYER_ROOT",
    "HOLD_KNOB_TEST": "PLAYER_ROOT",
    "VOLUME_RAIL_TEST": "PLAYER_ROOT",
    "VOLUME_KNOB_TEST": "PLAYER_ROOT",
})
POSITIONS.update({
    "BTN_MENU_TEST": (0.003, 0.013, 0.073),
    "HOLD_RAIL_TEST": (0.02, 0.013, 0.073),
    "HOLD_KNOB_TEST": (0.017, 0.013, 0.0735),
    "VOLUME_RAIL_TEST": (0.043, 0.013, 0.073),
    "VOLUME_KNOB_TEST": (0.043, 0.013, 0.0735),
})


def vector(line):
    match = VECTOR.search(line)
    if not match:
        raise ValueError(f"Expected Qt.vector3d: {line}")
    return tuple(float(value.strip()) for value in match.group(1).split(","))


def close(actual, expected):
    return len(actual) == len(expected) and all(
        math.isclose(a, b, abs_tol=1e-5, rel_tol=0)
        for a, b in zip(actual, expected, strict=True)
    )


def validate(path=ASSET):
    if not path.is_file():
        raise ValueError(f"Missing Balsam QML: {path}")
    nodes = {}
    stack = []
    scales = []
    rotations = []
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            continue
        if line.endswith("{"):
            stack.append({"kind": line.split()[0], "name": None})
        elif line == "}":
            if not stack:
                raise ValueError("Unbalanced QML braces")
            stack.pop()
        elif line.startswith("objectName:") and stack and stack[-1]["kind"] in {"Node", "Model"}:
            name = re.fullmatch(r'objectName: "([^"]+)"', line)
            if not name:
                raise ValueError(f"Malformed objectName: {line}")
            name = name.group(1)
            if name in nodes:
                raise ValueError(f"Duplicate objectName: {name}")
            parent = next(
                (entry["name"] for entry in reversed(stack[:-1]) if entry["name"]),
                None,
            )
            stack[-1]["name"] = name
            nodes[name] = {"parent": parent, "kind": stack[-1]["kind"], "position": (0, 0, 0), "source": None}
        elif line.startswith("position:") and stack and stack[-1]["name"]:
            nodes[stack[-1]["name"]]["position"] = vector(line)
        elif line.startswith("source:") and stack and stack[-1]["name"]:
            match = re.fullmatch(r'source: "([^"]+)"', line)
            if not match:
                raise ValueError(f"Malformed mesh source: {line}")
            nodes[stack[-1]["name"]]["source"] = match.group(1)
        elif line.startswith("scale:"):
            scales.append((len(stack), vector(line)))
        elif line.startswith(("rotation:", "eulerRotation:")):
            rotations.append(line)
    if stack:
        raise ValueError("Unbalanced QML braces")
    if set(nodes) != set(PARENTS):
        raise ValueError(f"Wrong named nodes: missing={sorted(set(PARENTS)-set(nodes))}, extra={sorted(set(nodes)-set(PARENTS))}")
    if scales != [(1, (100.0, 100.0, 100.0))] or rotations:
        raise ValueError(f"Unexpected scale or rotation: {scales}, {rotations}")
    mesh_count = 0
    for name, expected_parent in PARENTS.items():
        node = nodes[name]
        if node["parent"] != expected_parent:
            raise ValueError(f"{name}: wrong parent {node['parent']!r}")
        if not close(node["position"], POSITIONS.get(name, (0, 0, 0))):
            raise ValueError(f"{name}: wrong local position {node['position']}")
        expected_kind = "Node" if name in {"PLAYER_ROOT", "LID_ROOT", "DISC_ROOT"} else "Model"
        if node["kind"] != expected_kind:
            raise ValueError(f"{name}: wrong QML kind {node['kind']}")
        if expected_kind == "Model":
            source = node["source"]
            if not source or not (path.parent / source).is_file():
                raise ValueError(f"{name}: missing mesh {source!r}")
            mesh_count += 1
        elif node["source"] is not None:
            raise ValueError(f"{name}: pivot must not have a mesh")
    return {"status": "ok", "nodes": len(nodes), "meshes": mesh_count, "import_scale": 100}


if __name__ == "__main__":
    try:
        result = validate(Path(sys.argv[1]) if len(sys.argv) > 1 else ASSET)
    except ValueError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
    print(json.dumps(result, sort_keys=True))
