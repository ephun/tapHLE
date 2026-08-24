import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "dev-scripts" / "release_version.py"
SPEC = importlib.util.spec_from_file_location("taphle_release_version", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
release_version = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release_version)


class ReleaseVersionTests(unittest.TestCase):
    def test_repository_uses_the_first_0_2_4_development_version(self):
        version = release_version.workspace_version()
        self.assertEqual(version, "0.2.4-dev.1")
        release_version.validate_development_version(version)

    def test_development_version_format_is_bounded(self):
        for version in ["0.2.4-dev.1", "0.2.4-dev.27+g2b5b4089"]:
            with self.subTest(version=version):
                release_version.validate_development_version(version)

        for invalid in [
            "0.2.4-alpha.1",
            "0.2.4-beta.1",
            "0.2.4-rc.1",
            "0.2.4-dev.0",
            "0.2.4-dev.1+2b5b4089",
        ]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    release_version.validate_development_version(invalid)

    def test_numbered_releases_have_no_staging_suffix(self):
        release_version.validate_release_version("0.2.4")
        for invalid in [
            "0.2.4-dev.1",
            "0.2.4-alpha.1",
            "0.2.4-beta.1",
            "0.2.4-rc.1",
            "0.2.4+g2b5b4089",
            "00.2.4",
            "0.02.4",
            "0.2.04",
        ]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    release_version.validate_release_version(invalid)

    def test_release_artifact_names_cover_all_five_hosts(self):
        expected = {
            ("Windows", "x86_64", "zip"): "tapHLE-v0.2.4-Windows-x86_64.zip",
            ("Linux", "x86_64", "tar.gz"): "tapHLE-v0.2.4-Linux-x86_64.tar.gz",
            ("macOS", "x86_64", "dmg"): "tapHLE-v0.2.4-macOS-x86_64.dmg",
            ("Android", "arm64-v8a", "apk"): "tapHLE-v0.2.4-Android-arm64-v8a.apk",
            ("iOS", "arm64", "ipa"): "tapHLE-v0.2.4-iOS-arm64.ipa",
        }
        for (host, architecture, extension), name in expected.items():
            with self.subTest(host=host):
                self.assertEqual(
                    release_version.artifact_name(
                        "0.2.4", host, architecture, extension
                    ),
                    name,
                )

        self.assertEqual(
            release_version.windows_installer_name("0.2.4"),
            "tapHLE-v0.2.4-Windows-x86_64-setup.exe",
        )
        self.assertEqual(len(release_version.release_artifact_names("0.2.4")), 6)

        for invalid in [
            ("Plan9", "mips", "exe"),
            ("Windows", "arm64", "ipa"),
            ("Linux", "x86_64", "tar..gz"),
        ]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    release_version.artifact_name("0.2.4", *invalid)

    def test_frozen_0_2_4_cohort_is_exact_and_audited(self):
        path = REPOSITORY_ROOT / "compatibility" / "release-cohorts" / "0.2.4.json"
        cohort = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(cohort["release"], "0.2.4")
        self.assertEqual(cohort["freeze_date"], "2026-08-24")
        entries = cohort["cohort"]
        derived_counts = {
            "cohort_versions": len(entries),
            "approved_basis": sum(
                entry["report_moderation_status"] == "approved" for entry in entries
            ),
            "trusted_pending_basis": sum(
                entry["report_moderation_status"] == "pending" for entry in entries
            ),
        }
        self.assertEqual(cohort["counts"], derived_counts)
        self.assertEqual(derived_counts, {"cohort_versions": 24, "approved_basis": 22, "trusted_pending_basis": 2})
        self.assertTrue(all(entry["rating"] == 3 for entry in entries))
        self.assertTrue(
            all(
                entry["source_identity"] == "agent:claude-code"
                for entry in entries
                if entry["report_moderation_status"] == "pending"
            )
        )
        identities = {
            (entry["bundle_identifier"], entry["version_identity"]["bundle_version"])
            for entry in entries
        }
        self.assertEqual(len(identities), 24)
        self.assertTrue(all(entry["audit_passed"] for entry in entries))
        self.assertTrue(
            all(
                len(entry["tested_taphle_commit_full"]) == 40
                for entry in entries
            )
        )

    def test_workspace_version_reads_requested_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text(
                '[workspace.package]\nversion = "1.2.3-rc.4"\n',
                encoding="utf-8",
            )
            self.assertEqual(release_version.workspace_version(manifest), "1.2.3-rc.4")

    def test_tag_must_match_exactly(self):
        release_version.validate_tag("taphle-v0.2.4", "0.2.4")
        for invalid in [
            "v0.2.4",
            "taphle-v0.2.5",
            "taphle-v0.2.4-dev.1",
            "taphle-v0.2.4-dirty",
            "taphle-v0.2.4-1-gabc1234",
        ]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    release_version.validate_tag(invalid, "0.2.4")

    def test_changelog_requires_exact_version_and_valid_date(self):
        with tempfile.TemporaryDirectory() as directory:
            changelog = Path(directory) / "CHANGELOG.md"
            changelog.write_text(
                "# Changelog\n\n## 0.2.4 - 2026-07-18\n",
                encoding="utf-8",
            )
            release_version.validate_changelog("0.2.4", changelog)

            for invalid in [
                "## Unreleased for 0.2.4",
                "## 0.2.3 - 2026-07-18",
                "## 0.2.4 - 2026-02-30",
            ]:
                with self.subTest(invalid=invalid):
                    changelog.write_text(f"# Changelog\n\n{invalid}\n", encoding="utf-8")
                    with self.assertRaises(ValueError):
                        release_version.validate_changelog("0.2.4", changelog)


if __name__ == "__main__":
    unittest.main()
