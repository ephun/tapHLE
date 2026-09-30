#!/usr/bin/env python3
"""Build and exercise tapHLE on a connected physical iOS device."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_APP = (ROOT / "build/ios-iphoneos/Build/Products/"
               "Release-iphoneos/tapHLE.app")
DEFAULT_IPA = ROOT / "build/ios-iphoneos/tapHLE-iOS-arm64-Release.ipa"
BUNDLE_ID = "org.taphle.ios"

EXIT_CODES = {
    "OK": 0,
    "PHONE_NOT_CONNECTED": 20,
    "PHONE_LOCKED": 21,
    "PHONE_NOT_PAIRED": 22,
    "DEVELOPER_MODE_DISABLED": 23,
    "SIGNING_FAILED": 30,
    "INSTALL_FAILED": 31,
    "JIT_NOT_AVAILABLE": 32,
    "APP_LAUNCH_FAILED": 33,
    "APP_CRASHED": 34,
    "TEST_TIMEOUT": 35,
    "BUILD_FAILED": 40,
    "HARNESS_ERROR": 70,
}


class HarnessFailure(Exception):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code
        self.message = message


class Harness:
    def __init__(self, args: argparse.Namespace):
        self.args = args
        run_root = Path(os.environ.get(
            "TAPHLE_IOS_RUN_ROOT", ROOT / "build/ios-device-runs"))
        stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
        self.run_dir = (Path(args.run_dir) if args.run_dir else
                        run_root / f"{stamp}-{args.command}-{os.getpid()}")
        if self.run_dir.exists():
            raise HarnessFailure(
                "HARNESS_ERROR", f"Run directory already exists: {self.run_dir}")
        self.run_dir.mkdir(parents=True)
        self.command_index = 0
        self.summary: dict[str, object] = {
            "schema_version": 1,
            "command": args.command,
            "started_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "repository": str(ROOT),
            "commit": self.git_head(),
            "run_directory": str(self.run_dir),
        }

    def git_head(self) -> str:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
            capture_output=True, timeout=10, check=False)
        return result.stdout.strip() if result.returncode == 0 else "unknown"

    @staticmethod
    def tool(name: str, default: str) -> list[str]:
        return shlex.split(os.environ.get(name, default))

    def run(self, label: str, command: list[str], timeout: int,
            *, allow_timeout: bool = False,
            environment: dict[str, str] | None = None
            ) -> subprocess.CompletedProcess[str]:
        self.command_index += 1
        prefix = self.run_dir / f"{self.command_index:02d}-{label}"
        try:
            result = subprocess.run(
                command, cwd=ROOT, text=True, capture_output=True,
                timeout=timeout, check=False, env=environment)
        except subprocess.TimeoutExpired as error:
            stdout = error.stdout or ""
            stderr = error.stderr or ""
            if isinstance(stdout, bytes):
                stdout = stdout.decode(errors="replace")
            if isinstance(stderr, bytes):
                stderr = stderr.decode(errors="replace")
            prefix.with_suffix(".stdout.log").write_text(stdout)
            prefix.with_suffix(".stderr.log").write_text(stderr)
            if allow_timeout:
                return subprocess.CompletedProcess(command, 124, stdout, stderr)
            raise HarnessFailure(
                "TEST_TIMEOUT", f"{label} exceeded its {timeout}s timeout")
        prefix.with_suffix(".stdout.log").write_text(result.stdout)
        prefix.with_suffix(".stderr.log").write_text(result.stderr)
        return result

    def devicectl(self, label: str, arguments: list[str],
                  timeout: int | None = None) -> tuple[subprocess.CompletedProcess[str], dict]:
        timeout = timeout or self.args.command_timeout
        self.command_index += 1
        prefix = self.run_dir / f"{self.command_index:02d}-{label}"
        json_path = prefix.with_suffix(".json")
        log_path = prefix.with_suffix(".log")
        command = (self.tool("TAPHLE_DEVICECTL", "xcrun devicectl") +
                   ["--timeout", str(max(5, timeout)),
                    "--json-output", str(json_path),
                    "--log-output", str(log_path)] + arguments)
        try:
            result = subprocess.run(
                command, cwd=ROOT, text=True, capture_output=True,
                timeout=timeout + 5, check=False)
        except subprocess.TimeoutExpired as error:
            result = subprocess.CompletedProcess(
                command, 124, error.stdout or "", error.stderr or "")
        prefix.with_suffix(".stdout.log").write_text(result.stdout or "")
        prefix.with_suffix(".stderr.log").write_text(result.stderr or "")
        payload = {}
        if json_path.exists():
            try:
                payload = json.loads(json_path.read_text())
            except json.JSONDecodeError:
                payload = {}
        return result, payload

    @staticmethod
    def dictionaries(value):
        if isinstance(value, dict):
            yield value
            for child in value.values():
                yield from Harness.dictionaries(child)
        elif isinstance(value, list):
            for child in value:
                yield from Harness.dictionaries(child)

    @staticmethod
    def text(value) -> str:
        try:
            return json.dumps(value, sort_keys=True).lower()
        except TypeError:
            return str(value).lower()

    @staticmethod
    def plist_fragment(*values: str) -> dict:
        for value in values:
            encoded = value.encode()
            start = encoded.find(b"<?xml")
            end = encoded.find(b"</plist>", start)
            if start >= 0 and end >= 0:
                return plistlib.loads(encoded[start:end + len(b"</plist>")])
        raise ValueError("no XML property list found")

    @staticmethod
    def value_for(record: dict, *names: str):
        wanted = {name.lower() for name in names}
        for key, value in record.items():
            if key.lower() in wanted:
                return value
        return None

    def classify_device_error(self, value, fallback: str) -> HarnessFailure:
        message = self.text(value)
        if "developer mode" in message or "developermode" in message:
            return HarnessFailure(
                "DEVELOPER_MODE_DISABLED",
                "Enable Developer Mode in Settings > Privacy & Security, "
                "restart the iPhone, and confirm the prompt.")
        if "locked" in message or "passcode" in message:
            return HarnessFailure(
                "PHONE_LOCKED", "Unlock the iPhone and keep its screen awake.")
        if any(word in message for word in ("pair", "trust", "invalid host id")):
            return HarnessFailure(
                "PHONE_NOT_PAIRED",
                "Trust this Mac on the unlocked iPhone, then complete Xcode pairing.")
        return HarnessFailure(fallback, message[-500:] or fallback)

    @staticmethod
    def is_physical_ios(record: dict) -> bool:
        simulator = Harness.value_for(record, "simulator", "isSimulator")
        platform = Harness.value_for(record, "platform", "platformIdentifier")
        platform_text = str(platform or "").lower()
        product = Harness.value_for(record, "productType", "modelCode", "modelName")
        product_text = str(product or "").lower()
        ios_platform = ("iphoneos" in platform_text or platform_text == "ios" or
                        product_text.startswith(("iphone", "ipad", "ipod")))
        return simulator is not True and ios_platform

    def discover_device(self) -> dict:
        result, payload = self.devicectl("devices", ["list", "devices"])
        records = [record for record in self.dictionaries(payload)
                   if self.is_physical_ios(record) and self.value_for(
                       record, "identifier", "udid", "deviceIdentifier",
                       "coreDeviceIdentifier")]

        if not records:
            xcdevice = self.run(
                "xcdevice", self.tool("TAPHLE_XCDEVICE", "xcrun xcdevice") +
                ["list", "--timeout", str(self.args.command_timeout)],
                self.args.command_timeout + 5, allow_timeout=True)
            try:
                xc_payload = json.loads(xcdevice.stdout) if xcdevice.stdout else []
            except json.JSONDecodeError:
                xc_payload = []
            records = [record for record in self.dictionaries(xc_payload)
                       if self.is_physical_ios(record) and self.value_for(
                           record, "identifier", "udid", "deviceIdentifier",
                           "coreDeviceIdentifier")]

        selector = self.args.device
        if selector:
            records = [record for record in records
                       if selector.lower() in self.text(record)]
        if records:
            available = [record for record in records
                         if self.value_for(record, "available") is not False]
            record = (available or records)[0]
            identifier = self.value_for(
                record, "identifier", "udid", "deviceIdentifier", "coreDeviceIdentifier")
            if not identifier:
                raise HarnessFailure(
                    "PHONE_NOT_CONNECTED",
                    "Apple tooling found an iOS device but exposed no usable identifier.")
            record = dict(record)
            record["_selector"] = str(identifier)
            return record

        idevice = self.tool("TAPHLE_IDEVICE_ID", "/usr/local/bin/idevice_id")
        if shutil.which(idevice[0]):
            seen = self.run("idevice-id", idevice + ["-l"], 5,
                            allow_timeout=True)
            identifiers = [line.strip() for line in seen.stdout.splitlines()
                           if line.strip()]
            if identifiers:
                info_tool = self.tool(
                    "TAPHLE_IDEVICEINFO", "/usr/local/bin/ideviceinfo")
                info = self.run("idevice-info", info_tool +
                                ["-u", identifiers[0], "-k", "ProductVersion"],
                                5, allow_timeout=True)
                if info.returncode != 0:
                    raise self.classify_device_error(
                        info.stderr, "PHONE_NOT_PAIRED")
                raise HarnessFailure(
                    "DEVELOPER_MODE_DISABLED",
                    "The USB device is paired but unavailable to Xcode/CoreDevice; "
                    "enable Developer Mode and finish Xcode device preparation.")

        detail = self.text(payload) + (result.stderr or "")
        if any(word in detail for word in ("pair", "trust")):
            raise self.classify_device_error(detail, "PHONE_NOT_PAIRED")
        raise HarnessFailure(
            "PHONE_NOT_CONNECTED",
            "No physical iPhone is visible to devicectl, xcdevice, or idevice_id; "
            "connect USB passthrough before retrying.")

    def device_status(self) -> dict:
        record = self.discover_device()
        selector = record["_selector"]
        result, details = self.devicectl(
            "device-details", ["device", "info", "details", "--device", selector])
        if result.returncode != 0:
            raise self.classify_device_error(
                details or result.stderr, "PHONE_NOT_CONNECTED")
        result, lock = self.devicectl(
            "lock-state", ["device", "info", "lockState", "--device", selector])
        if result.returncode != 0:
            raise self.classify_device_error(lock or result.stderr, "PHONE_LOCKED")
        locked = None
        for state_record in self.dictionaries(lock):
            value = self.value_for(state_record, "locked", "isLocked")
            if isinstance(value, bool):
                locked = value
            unlocked = self.value_for(state_record, "unlocked", "isUnlocked")
            if isinstance(unlocked, bool):
                locked = not unlocked
        if locked is True:
            raise HarnessFailure(
                "PHONE_LOCKED", "Unlock the iPhone and keep its screen awake.")
        details_text = self.text(details)
        if ("developermodestatus" in details_text and
                any(value in details_text for value in
                    ("disabled", '"developermodestatus": false'))):
            raise HarnessFailure(
                "DEVELOPER_MODE_DISABLED",
                "Enable Developer Mode in Settings > Privacy & Security.")
        if ("pairingstate" in details_text and
                any(value in details_text for value in ("unpaired", "notpaired"))):
            raise HarnessFailure(
                "PHONE_NOT_PAIRED", "Trust this Mac and complete Xcode pairing.")
        safe = {
            "name": self.value_for(record, "name"),
            "model": self.value_for(record, "modelName", "productType"),
            "os": self.value_for(record, "operatingSystemVersion", "osVersion"),
            "identifier": selector,
        }
        self.summary["device"] = safe
        return {"selector": selector, "record": record, "details": details}

    def inspect_source_jit(self) -> dict:
        entitlement_path = ROOT / "platforms/ios/Config/TapHLE.entitlements"
        config_path = ROOT / "platforms/ios/Config/Build.xcconfig"
        with entitlement_path.open("rb") as handle:
            entitlements = plistlib.load(handle)
        config = config_path.read_text()
        valid = (entitlements.get("get-task-allow") is True and
                 "CODE_SIGN_ENTITLEMENTS = Config/TapHLE.entitlements" in config)
        if not valid:
            raise HarnessFailure(
                "JIT_NOT_AVAILABLE",
                "The device target does not request get-task-allow through its "
                "configured entitlements file.")
        return {
            "get_task_allow_requested": True,
            "device_side_mechanism": "StikDebug (implemented by the iOS host)",
            "runtime_proof": "Documents/jit-status.txt must contain Ready:",
        }

    def codesign_entitlements(self, app: Path) -> dict:
        result = self.run(
            "codesign-entitlements",
            self.tool("TAPHLE_CODESIGN", "codesign") +
            ["-d", "--entitlements", ":-", str(app)], 15)
        if result.returncode != 0:
            raise HarnessFailure(
                "SIGNING_FAILED", f"Could not read entitlements from {app}.")
        try:
            entitlements = self.plist_fragment(result.stdout, result.stderr)
        except Exception as error:
            raise HarnessFailure(
                "SIGNING_FAILED", f"Invalid signed entitlements: {error}")
        if entitlements.get("get-task-allow") is not True:
            raise HarnessFailure(
                "JIT_NOT_AVAILABLE",
                "The signed app lacks get-task-allow=true; re-sign while preserving it.")
        detail = self.run(
            "codesign-details", self.tool("TAPHLE_CODESIGN", "codesign") +
            ["-dv", "--verbose=4", str(app)], 15)
        signature_text = detail.stdout + detail.stderr
        ad_hoc = ("Signature=adhoc" in signature_text or
                  "TeamIdentifier=not set" in signature_text)
        return {"get_task_allow": True, "ad_hoc": ad_hoc,
                "details": signature_text[-2000:]}

    def validate_install_signature(self, app: Path) -> dict:
        if not app.is_dir():
            raise HarnessFailure("SIGNING_FAILED", f"App bundle not found: {app}")
        signature = self.codesign_entitlements(app)
        profile = app / "embedded.mobileprovision"
        if signature["ad_hoc"] or not profile.is_file():
            raise HarnessFailure(
                "SIGNING_FAILED",
                "The canonical build is ad-hoc signed and cannot be installed. "
                "Re-sign the .app with an Apple Development identity and embedded "
                "development provisioning profile, preserving get-task-allow=true.")
        decoded = self.run(
            "provisioning-profile",
            self.tool("TAPHLE_SECURITY", "security") +
            ["cms", "-D", "-i", str(profile)], 15)
        if decoded.returncode != 0:
            raise HarnessFailure(
                "SIGNING_FAILED", "The embedded provisioning profile is unreadable.")
        try:
            profile_data = plistlib.loads(decoded.stdout.encode())
        except Exception as error:
            raise HarnessFailure(
                "SIGNING_FAILED", f"Invalid provisioning profile: {error}")
        profile_entitlements = profile_data.get("Entitlements", {})
        if profile_entitlements.get("get-task-allow") is not True:
            raise HarnessFailure(
                "JIT_NOT_AVAILABLE",
                "The provisioning profile does not grant get-task-allow=true.")
        return signature

    def status(self):
        self.device_status()

    def prepare(self):
        self.device_status()
        self.summary["jit_prerequisites"] = self.inspect_source_jit()
        tools = {}
        for environment, default in (
                ("TAPHLE_DEVICECTL", "xcrun"),
                ("TAPHLE_XCDEVICE", "xcrun"),
                ("TAPHLE_CODESIGN", "codesign"),
                ("TAPHLE_IDEVICE_ID", "/usr/local/bin/idevice_id"),
                ("TAPHLE_IDEVICESYSLOG", "/usr/local/bin/idevicesyslog"),
                ("TAPHLE_IDEVICESCREENSHOT", "/usr/local/bin/idevicescreenshot")):
            command = self.tool(environment, default)[0]
            tools[environment] = shutil.which(command)
        self.summary["tools"] = tools
        missing = [name for name, path in tools.items() if not path]
        if missing:
            raise HarnessFailure(
                "HARNESS_ERROR", "Missing required tools: " + ", ".join(missing))
        app = Path(self.args.app) if self.args.app else DEFAULT_APP
        if app.exists():
            self.summary["app_signature"] = self.codesign_entitlements(app)
        else:
            self.summary["app_signature"] = "not built; run build before install"

    def build(self):
        started = time.time()
        environment = os.environ.copy()
        if not environment.get("TAPHLE_BOOST_ROOT"):
            for candidate in (Path("/usr/local/opt/boost/include"),
                              Path("/opt/homebrew/opt/boost/include")):
                if (candidate / "boost/version.hpp").is_file():
                    environment["TAPHLE_BOOST_ROOT"] = str(candidate)
                    break
        result = self.run(
            "build-host-release",
            ["sh", str(ROOT / "platforms/ios/scripts/build-host.sh"),
             "iphoneos", "Release", "--ipa"], self.args.build_timeout,
            environment=environment)
        if result.returncode != 0:
            error = result.stdout + result.stderr
            code = "SIGNING_FAILED" if "codesign" in error.lower() else "BUILD_FAILED"
            raise HarnessFailure(code, "Canonical iOS Release build failed; see run logs.")
        for path in (DEFAULT_APP, DEFAULT_IPA, DEFAULT_IPA.with_suffix(".json")):
            if not path.exists() or path.stat().st_mtime < started - 1:
                raise HarnessFailure(
                    "BUILD_FAILED", f"Canonical build did not produce a fresh {path}.")
        manifest = json.loads(DEFAULT_IPA.with_suffix(".json").read_text())
        digest_hash = hashlib.sha256()
        with DEFAULT_IPA.open("rb") as ipa:
            for chunk in iter(lambda: ipa.read(1024 * 1024), b""):
                digest_hash.update(chunk)
        digest = digest_hash.hexdigest()
        if manifest.get("profile") != "Release" or manifest.get("sha256") != digest:
            raise HarnessFailure(
                "BUILD_FAILED", "IPA manifest does not match the Release output.")
        signature = self.codesign_entitlements(DEFAULT_APP)
        self.summary["build"] = {
            "app": str(DEFAULT_APP), "ipa": str(DEFAULT_IPA),
            "ipa_sha256": digest, "manifest": manifest,
            "signature": signature,
        }

    def install(self):
        device = self.device_status()
        app = Path(self.args.app) if self.args.app else DEFAULT_APP
        self.summary["app_signature"] = self.validate_install_signature(app)
        result, payload = self.devicectl(
            "install", ["device", "install", "app", "--device",
                        device["selector"], str(app)], self.args.install_timeout)
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "INSTALL_FAILED")
        self.summary["installed_app"] = str(app)

    def process_records(self, selector: str) -> list[dict]:
        result, payload = self.devicectl(
            "processes", ["device", "info", "processes", "--device", selector])
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "PHONE_NOT_CONNECTED")
        matches = []
        for record in self.dictionaries(payload):
            text = self.text(record)
            if BUNDLE_ID.lower() in text or "taphle" in text:
                matches.append(record)
        return matches

    def process_pid(self, selector: str) -> int | None:
        for record in self.process_records(selector):
            value = self.value_for(record, "processIdentifier", "pid")
            if isinstance(value, int) or str(value).isdigit():
                return int(value)
        return None

    def launch(self):
        device = self.device_status()
        result, payload = self.devicectl(
            "launch", ["device", "process", "launch", "--device",
                       device["selector"], "--terminate-existing", BUNDLE_ID])
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "APP_LAUNCH_FAILED")
        time.sleep(2)
        pid = self.process_pid(device["selector"])
        if pid is None:
            raise HarnessFailure(
                "APP_LAUNCH_FAILED", "devicectl returned success but tapHLE is not running.")
        self.summary["pid"] = pid

    def stop(self):
        device = self.device_status()
        pid = self.process_pid(device["selector"])
        if pid is None:
            self.summary["already_stopped"] = True
            return
        result, payload = self.devicectl(
            "terminate", ["device", "process", "terminate", "--device",
                          device["selector"], "--pid", str(pid)])
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "APP_LAUNCH_FAILED")
        self.summary["stopped_pid"] = pid

    def logs(self):
        device = self.device_status()
        tool = self.tool(
            "TAPHLE_IDEVICESYSLOG", "/usr/local/bin/idevicesyslog")
        if not shutil.which(tool[0]):
            raise HarnessFailure(
                "HARNESS_ERROR", "idevicesyslog is required for bounded device logs.")
        output = self.run(
            "device-syslog", tool + ["-u", device["selector"],
                                     "--no-colors", "-p", "tapHLE"],
            self.args.duration, allow_timeout=True)
        log_path = self.run_dir / "taphle-device.log"
        log_path.write_text(output.stdout + output.stderr)
        self.summary["log"] = str(log_path)
        self.summary["duration_seconds"] = self.args.duration

    def crash_names(self, selector: str, *, copy: bool) -> set[str]:
        result, payload = self.devicectl(
            "crash-list", ["device", "info", "files", "--device", selector,
                           "--domain-type", "systemCrashLogs"])
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "PHONE_NOT_CONNECTED")
        names = set()
        for record in self.dictionaries(payload):
            name = self.value_for(record, "name", "filename", "relativePath", "path")
            if name and "taphle" in str(name).lower():
                names.add(str(name))
        if copy:
            destination = self.run_dir / "crashes"
            destination.mkdir()
            for number, name in enumerate(sorted(names), 1):
                copy_result, copy_payload = self.devicectl(
                    f"crash-copy-{number}", ["device", "copy", "from", "--device",
                        selector, "--domain-type", "systemCrashLogs", "--source",
                        name, "--destination", str(destination / Path(name).name)])
                if copy_result.returncode != 0:
                    raise self.classify_device_error(
                        copy_payload or copy_result.stderr, "HARNESS_ERROR")
            self.summary["crash_directory"] = str(destination)
        return names

    def crashes(self):
        device = self.device_status()
        names = self.crash_names(device["selector"], copy=True)
        self.summary["crashes"] = sorted(names)

    def screenshot(self):
        device = self.device_status()
        tool = self.tool(
            "TAPHLE_IDEVICESCREENSHOT", "/usr/local/bin/idevicescreenshot")
        if not shutil.which(tool[0]):
            raise HarnessFailure(
                "HARNESS_ERROR", "idevicescreenshot is required by Xcode 16.4.")
        path = self.run_dir / "screenshot.png"
        result = self.run(
            "screenshot", tool + ["-u", device["selector"], str(path)],
            self.args.command_timeout)
        if result.returncode != 0 or not path.exists():
            raise self.classify_device_error(
                result.stderr, "HARNESS_ERROR")
        self.summary["screenshot"] = str(path)

    def copy_jit_status(self, selector: str) -> str | None:
        destination = self.run_dir / "jit-status.txt"
        result, _ = self.devicectl(
            "jit-status", ["device", "copy", "from", "--device", selector,
                           "--domain-type", "appDataContainer",
                           "--domain-identifier", BUNDLE_ID,
                           "--source", "Documents/jit-status.txt",
                           "--destination", str(destination)])
        if result.returncode != 0 or not destination.exists():
            return None
        return destination.read_text(errors="replace")

    def test(self):
        device = self.device_status()
        before = self.crash_names(device["selector"], copy=False)
        pid = self.launch_on(device["selector"])
        deadline = time.monotonic() + self.args.duration
        while time.monotonic() < deadline:
            time.sleep(min(2, max(0, deadline - time.monotonic())))
            if self.process_pid(device["selector"]) is None:
                after = self.crash_names(device["selector"], copy=True)
                if after - before:
                    raise HarnessFailure(
                        "APP_CRASHED", "tapHLE exited and produced a new crash report.")
                raise HarnessFailure(
                    "APP_CRASHED", "tapHLE exited before the test interval completed.")
        jit = self.copy_jit_status(device["selector"])
        if not jit:
            raise HarnessFailure(
                "JIT_NOT_AVAILABLE",
                "No jit-status.txt was produced. During the bounded test, unlock "
                "the phone, press Play, complete the existing StikDebug handoff, "
                "and return to tapHLE.")
        self.summary["jit_status"] = jit
        if f"pid={pid}" not in jit:
            raise HarnessFailure(
                "JIT_NOT_AVAILABLE",
                "jit-status.txt belongs to an earlier tapHLE process; press Play "
                "and complete JIT preparation in the current process.")
        if "Ready:" not in jit:
            if "Requesting " in jit or "Preparing " in jit:
                raise HarnessFailure(
                    "TEST_TIMEOUT", "JIT preparation did not finish before the test deadline.")
            raise HarnessFailure("JIT_NOT_AVAILABLE", jit.strip().splitlines()[-1])
        self.summary["test_duration_seconds"] = self.args.duration

    def launch_on(self, selector: str) -> int:
        result, payload = self.devicectl(
            "test-launch", ["device", "process", "launch", "--device", selector,
                            "--terminate-existing", BUNDLE_ID])
        if result.returncode != 0:
            raise self.classify_device_error(
                payload or result.stderr, "APP_LAUNCH_FAILED")
        time.sleep(2)
        pid = self.process_pid(selector)
        if pid is None:
            raise HarnessFailure(
                "APP_LAUNCH_FAILED", "tapHLE did not remain running after launch.")
        return pid

    def cycle(self):
        self.device_status()
        self.summary["jit_prerequisites"] = self.inspect_source_jit()
        if not self.args.skip_build:
            self.build()
        self.install()
        self.test()

    def finish(self, code: str, message: str):
        self.summary.update({
            "finished_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "result": code,
            "message": message,
            "exit_code": EXIT_CODES[code],
        })
        summary_path = self.run_dir / "summary.json"
        summary_path.write_text(json.dumps(self.summary, indent=2, sort_keys=True) + "\n")
        print(json.dumps(self.summary, sort_keys=True))


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--device", help="device UDID, CoreDevice UUID, or name")
    result.add_argument("--run-dir", help="new directory for this run's artifacts")
    result.add_argument("--command-timeout", type=int, default=15,
                        help="per-device-command timeout in seconds (default: 15)")
    result.add_argument("--build-timeout", type=int, default=1800,
                        help="build timeout in seconds (default: 1800)")
    result.add_argument("--install-timeout", type=int, default=120,
                        help="install timeout in seconds (default: 120)")
    result.add_argument("--duration", type=int, default=30,
                        help="bounded logs/test duration in seconds (default: 30)")
    result.add_argument("--app", help="provisioned .app to inspect or install")
    result.add_argument("--skip-build", action="store_true",
                        help="cycle: use --app/current output without rebuilding")
    result.add_argument("command", choices=(
        "status", "prepare", "build", "install", "launch", "stop", "logs",
        "crashes", "screenshot", "test", "cycle"))
    return result


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    if min(args.command_timeout, args.build_timeout,
           args.install_timeout, args.duration) < 1:
        parser().error("timeouts and duration must be positive")
    harness = None
    try:
        harness = Harness(args)
        getattr(harness, args.command)()
        harness.finish("OK", f"{args.command} completed")
        return 0
    except HarnessFailure as error:
        if harness:
            harness.finish(error.code, error.message)
        else:
            print(json.dumps({"result": error.code, "message": error.message,
                              "exit_code": EXIT_CODES[error.code]}))
        return EXIT_CODES[error.code]
    except Exception as error:
        message = f"Unexpected harness error: {type(error).__name__}: {error}"
        if harness:
            harness.finish("HARNESS_ERROR", message)
        else:
            print(json.dumps({"result": "HARNESS_ERROR", "message": message,
                              "exit_code": EXIT_CODES["HARNESS_ERROR"]}))
        return EXIT_CODES["HARNESS_ERROR"]


if __name__ == "__main__":
    sys.exit(main())
