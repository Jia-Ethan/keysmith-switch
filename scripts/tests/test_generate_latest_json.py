from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "generate-latest-json.py"


class GenerateLatestJsonTests(unittest.TestCase):
    def make_artifact(self, root: Path, content: bytes = b"signed updater payload") -> Path:
        artifact = root / "Keysmith Switch.app.tar.gz"
        artifact.write_bytes(content)
        Path(f"{artifact}.sig").write_text("trusted comment: fixture\nsignature\n")
        return artifact

    def run_generator(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_writes_minimum_version_and_exact_artifact_size(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = self.make_artifact(root)
            output = root / "latest.json"

            result = self.run_generator(
                "--version",
                "0.1.4-rc.1",
                "--minimum-updater-version",
                "0.1.3",
                "--base-url",
                "https://example.test/releases/download/v0.1.4-rc.1",
                "--darwin-aarch64",
                str(artifact),
                "--out",
                str(output),
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            metadata = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(metadata["minimum_updater_version"], "0.1.3")
            self.assertEqual(
                metadata["platforms"]["darwin-aarch64"]["size"],
                artifact.stat().st_size,
            )

    def test_requires_minimum_updater_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = self.make_artifact(root)

            result = self.run_generator(
                "--version",
                "0.1.4-rc.1",
                "--base-url",
                "https://example.test/releases/download/v0.1.4-rc.1",
                "--darwin-aarch64",
                str(artifact),
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--minimum-updater-version", result.stderr)

    def test_rejects_invalid_minimum_version_and_empty_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = self.make_artifact(root)
            common = (
                "--version",
                "0.1.4-rc.1",
                "--base-url",
                "https://example.test/releases/download/v0.1.4-rc.1",
                "--darwin-aarch64",
                str(artifact),
            )

            invalid_version = self.run_generator(
                *common, "--minimum-updater-version", "0.1"
            )
            self.assertNotEqual(invalid_version.returncode, 0)
            self.assertIn("complete semantic version", invalid_version.stderr)

            artifact.write_bytes(b"")
            empty_artifact = self.run_generator(
                *common, "--minimum-updater-version", "0.1.3"
            )
            self.assertNotEqual(empty_artifact.returncode, 0)
            self.assertIn("artifact is empty", empty_artifact.stderr)


if __name__ == "__main__":
    unittest.main()
