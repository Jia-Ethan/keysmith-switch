import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "check_version", Path(__file__).resolve().parents[1] / "check-version.py"
)
check_version = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_version)


class LegacyNotePatternTest(unittest.TestCase):
    def test_flags_the_first_public_build(self):
        for text in ("v0.1.1 仍须手动安装", "版本 0.1.1", "`v0.1.1`"):
            self.assertTrue(check_version.LEGACY_NOTE_PATTERN.search(text), text)

    def test_ignores_longer_version_numbers(self):
        for text in ("v0.1.10", "v0.1.11", "v10.1.1", "0.1.1.2", "v0.1.3", "v0.2.1"):
            self.assertFalse(check_version.LEGACY_NOTE_PATTERN.search(text), text)

    def test_current_release_text_is_clean(self):
        self.assertEqual(check_version.legacy_note_mentions(), [])


if __name__ == "__main__":
    unittest.main()
