from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ClientReportingTests(unittest.TestCase):
    def test_unfinished_reporting_is_disabled(self):
        compat = (ROOT / "crates/gui/src/state/compat.rs").read_text(encoding="utf-8")
        self.assertRegex(
            compat,
            r"pub const CLIENT_REPORTING_AVAILABLE: bool = false;",
        )

    def test_every_product_entry_point_is_guarded(self):
        paths = [
            "crates/gui/src/app.rs",
            "crates/gui/src/ui/desktop/chrome.rs",
            "crates/gui/src/ui/desktop/details.rs",
            "crates/gui/src/ui/desktop/dialogs.rs",
            "crates/gui/src/ui/desktop/library_view.rs",
        ]
        for relative in paths:
            text = (ROOT / relative).read_text(encoding="utf-8")
            occurrences = [match.start() for match in re.finditer("OpenCompatibilityReport", text)]
            for position in occurrences:
                with self.subTest(path=relative, position=position):
                    self.assertIn(
                        "CLIENT_REPORTING_AVAILABLE",
                        text[max(0, position - 700) : position + 100],
                    )


if __name__ == "__main__":
    unittest.main()
