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
        self.assertEqual(result["nodes"], 14)
        self.assertEqual(result["meshes"], 11)

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
