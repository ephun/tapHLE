from pathlib import Path
import os
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "platforms" / "linux" / "make-bundle.sh"


class LinuxBundleTests(unittest.TestCase):
    def test_portable_bundle_contains_the_complete_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            working = Path(directory)
            binary = working / "tapHLE"
            binary.write_text("test executable", encoding="utf-8")
            binary.chmod(0o755)

            subprocess.run(
                [str(SCRIPT), str(binary)],
                cwd=working,
                check=True,
                text=True,
                capture_output=True,
            )

            bundle = working / "tapHLE_linux_bundle"
            required = [
                "tapHLE",
                "dylibs",
                "fonts",
                "apps/README.txt",
                "res/icon.png",
                "README.md",
                "CHANGELOG.md",
                "COPYING.txt",
                "OPTIONS_HELP.txt",
                "default_options.txt",
                "options.txt",
            ]
            for relative in required:
                with self.subTest(relative=relative):
                    self.assertTrue((bundle / relative).exists())
            self.assertTrue(os.access(bundle / "tapHLE", os.X_OK))
            self.assertEqual((bundle / "tapHLE").read_text(), "test executable")

            sources = {
                "dylibs": ROOT / "runtime/dylibs",
                "fonts": ROOT / "runtime/fonts",
                "apps/README.txt": ROOT / "runtime/apps/README.txt",
                "res/icon.png": ROOT / "runtime/res/icon.png",
                "README.md": ROOT / "README.md",
                "CHANGELOG.md": ROOT / "CHANGELOG.md",
                "COPYING.txt": ROOT / "dev-scripts/gpl-3.0.txt",
                "OPTIONS_HELP.txt": ROOT / "runtime/OPTIONS_HELP.txt",
                "default_options.txt": ROOT / "runtime/default_options.txt",
                "options.txt": ROOT / "runtime/options.txt",
            }
            for relative, source in sources.items():
                destination = bundle / relative
                if source.is_dir():
                    source_files = sorted(
                        item.relative_to(source) for item in source.rglob("*") if item.is_file()
                    )
                    destination_files = sorted(
                        item.relative_to(destination)
                        for item in destination.rglob("*")
                        if item.is_file()
                    )
                    self.assertEqual(source_files, destination_files)
                    for child in source_files:
                        self.assertEqual((source / child).read_bytes(), (destination / child).read_bytes())
                else:
                    self.assertEqual(source.read_bytes(), destination.read_bytes())

            archive = working / "tapHLE_Linux_x86_64.tar.gz"
            subprocess.run(
                ["tar", "-czf", str(archive), "tapHLE_linux_bundle"],
                cwd=working,
                check=True,
            )
            extracted = working / "extracted"
            extracted.mkdir()
            subprocess.run(
                ["tar", "-xzf", str(archive), "-C", str(extracted)],
                check=True,
            )
            self.assertTrue(os.access(extracted / "tapHLE_linux_bundle/tapHLE", os.X_OK))


if __name__ == "__main__":
    unittest.main()
