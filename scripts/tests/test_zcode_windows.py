"""Run the vendored ZCode sidecar as if on Windows (fake winreg, CRLF text mode, LOCALAPPDATA
discovery) so the Windows path is exercised by CI on any host. Real Windows still needs a smoke
test; this catches logic errors in install / doctor / uninstall and the registry round trip."""

import builtins
import importlib.util
import io
import json
import os
import pathlib
import platform
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SIDECAR = ROOT / "third_party" / "keysmith" / "zcode" / "zcode-keysmith.py"
ANCHOR = "x;customSystemPrompt:this.config.systemPrompt,language:y;"


def fake_winreg(store):
    mod = types.ModuleType("winreg")
    mod.HKEY_CURRENT_USER, mod.HKEY_LOCAL_MACHINE = "HKCU", "HKLM"
    mod.REG_SZ, mod.REG_EXPAND_SZ, mod.KEY_SET_VALUE = 1, 2, 2

    class Key:
        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

    mod.OpenKey = lambda *a, **k: Key()
    mod.CreateKeyEx = lambda *a, **k: Key()

    def query(_key, name):
        if name not in store:
            raise FileNotFoundError(name)
        return store[name]

    def delete(_key, name):
        if name not in store:
            raise FileNotFoundError(name)
        del store[name]

    mod.QueryValueEx = query
    mod.SetValueEx = lambda _k, name, _r, kind, value: store.__setitem__(name, (value, kind))
    mod.DeleteValue = delete
    return mod


class ZcodeWindowsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = pathlib.Path(self.tmp.name)
        self.local = root / "LocalAppData"
        self.app = self.local / "Programs" / "ZCode"
        (self.app / "resources" / "glm").mkdir(parents=True)
        (self.app / "ZCode.exe").write_bytes(b"MZ")
        (self.app / "resources" / "glm" / "zcode.cjs").write_text(ANCHOR, encoding="utf-8")
        self.managed = root / "home" / ".zcode-keysmith"
        self.prompt = root / "prompt.md"
        self.prompt.write_bytes(b"line one\nline two\n")
        self.registry = {}

        real_open, real_ntf = io.open, tempfile.NamedTemporaryFile

        def crlf_open(file, mode="r", buffering=-1, encoding=None, errors=None, newline=None, *a, **k):
            if "b" not in mode and any(c in mode for c in "wax") and newline is None:
                newline = "\r\n"
            return real_open(file, mode, buffering, encoding, errors, newline, *a, **k)

        def crlf_ntf(mode="w+b", buffering=-1, encoding=None, newline=None, *a, **k):
            if "b" not in mode and newline is None:
                newline = "\r\n"
            return real_ntf(mode, buffering, encoding, newline, *a, **k)

        def crlf_write_text(path, data, encoding=None, errors=None, newline=None):
            with crlf_open(path, "w", encoding=encoding, errors=errors, newline=newline) as handle:
                return handle.write(data)

        patches = [
            mock.patch.dict(os.environ, {"LOCALAPPDATA": str(self.local)}),
            mock.patch.dict(sys.modules, {"winreg": fake_winreg(self.registry)}),
            mock.patch.object(platform, "system", lambda: "Windows"),
            mock.patch.object(builtins, "open", crlf_open),
            mock.patch.object(io, "open", crlf_open),
            mock.patch.object(tempfile, "NamedTemporaryFile", crlf_ntf),
            mock.patch.object(pathlib.Path, "write_text", crlf_write_text),
        ]
        os.environ.pop("ZCODE_APP_PATH", None)
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)

        spec = importlib.util.spec_from_file_location("zcode_keysmith_win", SIDECAR)
        self.zk = importlib.util.module_from_spec(spec)
        sys.modules["zcode_keysmith_win"] = self.zk
        self.addCleanup(sys.modules.pop, "zcode_keysmith_win", None)
        spec.loader.exec_module(self.zk)
        # ctypes.WinDLL / windll do not exist off Windows.
        self.zk.windows_running_zcode_paths = lambda: []
        self.zk.broadcast_windows_environment_change = lambda: None

    def run_cli(self, *args):
        out, old = io.StringIO(), sys.stdout
        sys.stdout = out
        try:
            code = self.zk.main(list(args))
        except SystemExit as exc:
            code = exc.code
        finally:
            sys.stdout = old
        return code, out.getvalue()

    def test_discovers_zcode_under_localappdata(self):
        self.assertEqual(self.zk.discover_zcode_app_path(), self.app.resolve())

    def test_install_doctor_uninstall_round_trip(self):
        base = ("--managed-dir", str(self.managed))
        code, _ = self.run_cli("install", "--system-file", str(self.prompt), *base, "--dry-run")
        self.assertEqual(code, 0)
        self.assertFalse(self.managed.exists(), "a dry run writes nothing")

        code, _ = self.run_cli("install", "--system-file", str(self.prompt), *base, "--yes")
        self.assertEqual(code, 0)
        self.assertIn("ZCODE_KEYSMITH_SYSTEM_FILE", self.registry)
        self.assertTrue((self.managed / "bin" / "zcode-keysmith-env.ps1").is_file())
        self.assertFalse((self.managed / "bin" / "zcode-keysmith-env.sh").exists())
        config = json.loads((self.managed / "config.json").read_text(encoding="utf-8"))
        self.assertEqual(config["platform"], "Windows")
        self.assertIsNone(config["launch_agent"])
        # Windows points the app server at the (frozen) CLI itself, which re-enters the wrapper.
        wrapper = self.managed / "bin" / "zcode-agent-wrapper.py"
        self.assertEqual(pathlib.Path(json.loads(config["agent_server_args_json"])[0]).resolve(), wrapper.resolve())

        code, out = self.run_cli("doctor", *base, "--json")
        report = json.loads(out)
        self.assertEqual(report["blockers"], [], report)
        self.assertTrue(report["managed"]["system_file_exists"])

        code, _ = self.run_cli("uninstall", *base, "--yes", "--json")
        self.assertEqual(code, 0)
        self.assertEqual(self.registry, {}, "the user environment is restored")
        self.assertFalse((self.managed / "system-role.md").exists())

    @unittest.expectedFailure
    def test_user_node_options_survive_install_and_uninstall(self):
        # Known upstream behaviour (zcode-keysmith 0.3.2): install clears the user's NODE_OPTIONS
        # as a "stale" key and uninstall does not put it back. Vendored code is not edited here;
        # remove expectedFailure when the pin moves to a fixed upstream.
        self.registry["NODE_OPTIONS"] = ("--max-old-space-size=4096", 1)
        base = ("--managed-dir", str(self.managed))
        self.run_cli("install", "--system-file", str(self.prompt), *base, "--yes")
        self.run_cli("uninstall", *base, "--yes", "--json")
        self.assertEqual(self.registry.get("NODE_OPTIONS"), ("--max-old-space-size=4096", 1))

    def test_other_user_environment_comes_back(self):
        self.registry["ZCODE_KEYSMITH_CACHE_DIR"] = ("C:\\old-cache", 1)
        base = ("--managed-dir", str(self.managed))
        self.run_cli("install", "--system-file", str(self.prompt), *base, "--yes")
        self.run_cli("uninstall", *base, "--yes", "--json")
        self.assertEqual(self.registry.get("ZCODE_KEYSMITH_CACHE_DIR"), ("C:\\old-cache", 1))

    def test_windows_writes_crlf_and_the_host_must_cope(self):
        # The sidecar's own text-mode write turns LF into CRLF, so its reported hash is of the
        # CRLF bytes. The Rust side hashes LF text for ZCode (harness::live_body_text).
        base = ("--managed-dir", str(self.managed))
        self.run_cli("install", "--system-file", str(self.prompt), *base, "--yes")
        self.assertEqual((self.managed / "system-role.md").read_bytes(), b"line one\r\nline two\r\n")


if __name__ == "__main__":
    unittest.main()
