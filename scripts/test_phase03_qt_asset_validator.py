"""Positive and negative checks for the active controls QML asset."""

from pathlib import Path
import tempfile
import unittest
import sys
import re

sys.dont_write_bytecode = True
import validate_phase03_qt_asset as validator


class ControlsAssetTests(unittest.TestCase):
    def test_controls_present(self):
        result = validator.validate()
        self.assertEqual(result["nodes"], 19)
        self.assertEqual(result["meshes"], 16)

    def test_new_handle_mutations_rejected(self):
        source = validator.ASSET.read_text()
        for name in ("BTN_MENU_TEST", "HOLD_RAIL_TEST", "HOLD_KNOB_TEST", "VOLUME_RAIL_TEST", "VOLUME_KNOB_TEST"):
            self.assertIn(f'objectName: "{name}"', source)
            block = re.search(r'        Model \{\n            id: [^\n]+\n            objectName: "' + name + r'"\n.*?\n        \}', source, re.S).group()
            without = source.replace(block, "")
            mutants = [source.replace(f'objectName: "{name}"', f'objectName: "{replacement}"')
                       for replacement in ("MISSING", "BTN_PLAY_TEST")]
            mutants.extend([
                source.replace(block, block.replace("Model {", "Node {", 1)),
                source.replace(block, re.sub(r'position: Qt.vector3d\([^\n]+\)', 'position: Qt.vector3d(1, 2, 3)', block)),
                without.replace('        objectName: "LID_ROOT"', '        objectName: "LID_ROOT"\n' + block),
            ])
            for mutant in mutants:
                with tempfile.TemporaryDirectory() as tmp:
                    path = Path(tmp) / "asset.qml"
                    (Path(tmp) / "meshes").symlink_to((validator.ASSET.parent / "meshes").resolve(), target_is_directory=True)
                    path.write_text(mutant)
                    with self.assertRaises(ValueError):
                        validator.validate(path)

    def test_invalid_control_contract_rejected(self):
        source = validator.ASSET.read_text()
        self.assertIn('objectName: "BTN_PREVIOUS_TEST"', source)
        mutations = []
        for old, new in (
            ('objectName: "BTN_PREVIOUS_TEST"', 'objectName: "WRONG_BUTTON"'),
            ('objectName: "BTN_PREVIOUS_TEST"', 'objectName: "BTN_NEXT_TEST"'),
            ('Qt.vector3d(-0.033, 0.013, 0.073)', 'Qt.vector3d(-0.033, 0.013, 0.069)'),
        ):
            self.assertIn(old, source)
            mutations.append(source.replace(old, new))
        block = re.search(r'        Model \{\n            id: btn_PREVIOUS_TEST\n.*?\n        \}', source, re.S).group()
        without = source.replace(block, "")
        mutations.append(without.replace('        objectName: "LID_ROOT"', '        objectName: "LID_ROOT"\n' + block))
        mutations.append(source.replace('meshes/btn_PREVIOUS_TEST_MESH_mesh.mesh', 'meshes/missing.mesh'))
        for mutant in mutations:
            with self.subTest(mutant=mutant[:30]), tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp) / "asset.qml"
                (Path(tmp) / "meshes").symlink_to((validator.ASSET.parent / "meshes").resolve(), target_is_directory=True)
                path.write_text(mutant)
                with self.assertRaises(ValueError):
                    validator.validate(path)


if __name__ == "__main__":
    unittest.main()
