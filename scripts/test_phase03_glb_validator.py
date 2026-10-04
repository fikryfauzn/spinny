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
