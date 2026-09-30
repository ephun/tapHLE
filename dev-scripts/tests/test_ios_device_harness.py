"""No-device unit and contract tests for the physical-iOS harness."""

import importlib.util
import json
import os
from pathlib import Path
import py_compile
import stat
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "platforms/ios/scripts/ios-device.py"
SPEC = importlib.util.spec_from_file_location("ios_device_harness", SCRIPT)
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)


class IOSDeviceHarnessTests(unittest.TestCase):
    def make_tool(self, directory: Path, name: str, body: str) -> Path:
        path = directory / name
        path.write_text("#!/bin/sh\nset -eu\n" + body)
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def test_python_syntax(self):
        with tempfile.TemporaryDirectory(prefix="taphle-ios-device-syntax-") as folder:
            py_compile.compile(str(SCRIPT), cfile=str(Path(folder) / "harness.pyc"),
                               doraise=True)

    def test_only_explicit_ios_device_fields_count_as_a_phone(self):
        self.assertFalse(HARNESS.Harness.is_physical_ios({
            "path": "/tmp/build/ios-device-runs", "platform": "macOS"}))
        self.assertFalse(HARNESS.Harness.is_physical_ios({
            "modelName": "iPhone 15", "simulator": True}))
        self.assertTrue(HARNESS.Harness.is_physical_ios({
            "modelName": "iPhone 15", "simulator": False}))

    def test_codesign_plist_ignores_diagnostics_on_the_other_stream(self):
        xml = ('<?xml version="1.0"?><plist version="1.0"><dict>'
               '<key>get-task-allow</key><true/></dict></plist>')
        parsed = HARNESS.Harness.plist_fragment(
            xml, "Executable=/tmp/tapHLE\nwarning: deprecated")
        self.assertIs(parsed["get-task-allow"], True)

    def test_command_and_failure_code_contract(self):
        source = SCRIPT.read_text()
        for command in ("status", "prepare", "build", "install", "launch", "stop",
                        "logs", "crashes", "screenshot", "test", "cycle"):
            self.assertIn(f'"{command}"', source)
        expected = {
            "PHONE_NOT_CONNECTED": 20, "PHONE_LOCKED": 21,
            "PHONE_NOT_PAIRED": 22, "DEVELOPER_MODE_DISABLED": 23,
            "SIGNING_FAILED": 30, "INSTALL_FAILED": 31,
            "JIT_NOT_AVAILABLE": 32, "APP_LAUNCH_FAILED": 33,
            "APP_CRASHED": 34, "TEST_TIMEOUT": 35,
        }
        for name, number in expected.items():
            self.assertIn(f'"{name}": {number}', source)

    def test_build_uses_canonical_release_ipa_procedure(self):
        source = SCRIPT.read_text()
        self.assertIn('"platforms/ios/scripts/build-host.sh"', source)
        self.assertIn('"iphoneos", "Release", "--ipa"', source)
        self.assertIn('DEFAULT_IPA.with_suffix(".json")', source)

    def test_jit_contract_checks_source_and_signed_entitlements(self):
        source = SCRIPT.read_text()
        self.assertIn('entitlements.get("get-task-allow") is True', source)
        self.assertIn('profile_entitlements.get("get-task-allow") is not True', source)
        self.assertIn('"Documents/jit-status.txt"', source)
        self.assertIn('f"pid={pid}" not in jit', source)
        self.assertNotIn("stikdebug://", source)

    def test_no_device_status_is_json_and_stable_failure(self):
        with tempfile.TemporaryDirectory(prefix="taphle-ios-device-test-") as folder:
            directory = Path(folder)
            devicectl = self.make_tool(directory, "devicectl", '''
json=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = --json-output ]; then json=$2; shift 2; else shift; fi
done
printf '%s\n' '{"info":{"outcome":"success"},"result":{"devices":[]}}' > "$json"
''')
            xcdevice = self.make_tool(directory, "xcdevice", "printf '%s\\n' '[]'\n")
            idevice_id = self.make_tool(directory, "idevice_id", "exit 0\n")
            run_root = directory / "runs"
            environment = {
                **os.environ,
                "TAPHLE_DEVICECTL": str(devicectl),
                "TAPHLE_XCDEVICE": str(xcdevice),
                "TAPHLE_IDEVICE_ID": str(idevice_id),
                "TAPHLE_IOS_RUN_ROOT": str(run_root),
            }
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--command-timeout", "1", "status"],
                cwd=ROOT, env=environment, text=True, capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 20, result.stderr)
            summary = json.loads(result.stdout)
            self.assertEqual(summary["result"], "PHONE_NOT_CONNECTED")
            self.assertEqual(summary["exit_code"], 20)
            on_disk = json.loads(Path(summary["run_directory"], "summary.json").read_text())
            self.assertEqual(on_disk["result"], "PHONE_NOT_CONNECTED")


if __name__ == "__main__":
    unittest.main()
