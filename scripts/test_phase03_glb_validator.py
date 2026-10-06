"""Negative regression checks for the exported GLB validator."""

import copy
from pathlib import Path
import sys
from unittest.mock import patch

sys.dont_write_bytecode = True
import validate_phase03_glb as validator  # noqa: E402


path = Path("assets/runtime/phase03-greybox.glb")
document = validator.read_glb_json(path)
assert validator.validate(path)["status"] == "ok"
assert validator.validate(path)["nodes"] == 19, "Complete physical controls are required"

for name in ("BTN_PREVIOUS_TEST", "BTN_NEXT_TEST", "BTN_MENU_TEST", "HOLD_RAIL_TEST", "HOLD_KNOB_TEST", "VOLUME_RAIL_TEST", "VOLUME_KNOB_TEST"):
    for kind in ("missing", "duplicate", "position", "parent", "type"):
        mutant = copy.deepcopy(document)
        index = next(i for i, node in enumerate(mutant["nodes"]) if node.get("name") == name)
        if kind == "missing":
            mutant["nodes"][index]["name"] = "WRONG_BUTTON"
        elif kind == "duplicate":
            mutant["nodes"].append(copy.deepcopy(mutant["nodes"][index]))
        elif kind == "position":
            mutant["nodes"][index]["translation"] = [0, 0, 0]
        elif kind == "type":
            mutant["nodes"][index].pop("mesh", None)
        else:
            for node in mutant["nodes"]:
                if index in node.get("children", []):
                    node["children"].remove(index)
            next(node for node in mutant["nodes"] if node.get("name") == "LID_ROOT").setdefault("children", []).append(index)
        with patch.object(validator, "read_glb_json", return_value=mutant):
            try:
                validator.validate(path)
            except ValueError:
                pass
            else:
                raise AssertionError(f"Invalid {name} {kind} passed")

mutant = copy.deepcopy(document)
root = next(node for node in mutant["nodes"] if node.get("name") == "PLAYER_ROOT")
root["matrix"] = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 100, 0, 0, 1]
with patch.object(validator, "read_glb_json", return_value=mutant):
    try:
        validator.validate(path)
    except ValueError as error:
        assert "matrix transform" in str(error), error
    else:
        raise AssertionError("A hidden root matrix transform passed validation")

print("GLB validator negative tests: ok")
