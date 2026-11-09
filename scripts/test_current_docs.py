"""Current operation guides must not revive retired build directory policies."""
from pathlib import Path
import unittest

from doc_build_policy import CURRENT_GUIDES, validate_build_guidance


class BuildGuidanceTests(unittest.TestCase):
    def test_current_guides_follow_the_caller_and_metadata(self):
        root = Path(__file__).resolve().parents[1]
        for relative in CURRENT_GUIDES:
            with self.subTest(guide=relative):
                validate_build_guidance((root / relative).read_text(encoding="utf-8"), relative)

    def test_old_rules_fail_even_when_new_guidance_is_also_present(self):
        valid = ("Builds inherit the caller environment. Query cargo metadata "
                 "for target_directory in the owning workspace.")
        validate_build_guidance(valid, "fixture")
        for obsolete in ("D:/BuildCache/task", r"D:\BuildCache\task",
                         "/mnt/d/BuildCache/task", ".codex/build-targets/task",
                         "Set all Cargo/cache/temp paths on D:",
                         "The bin launchers set Cargo caches and temporary paths",
                         "Native Unix launchers require the verified D: mount"):
            with self.subTest(rule=obsolete), self.assertRaises(ValueError):
                validate_build_guidance(valid + "\n" + obsolete, "fixture")

    def test_historical_reports_are_outside_the_current_guide_allowlist(self):
        self.assertNotIn("docs/project-quality-review-2026-10-30.md", CURRENT_GUIDES)


if __name__ == "__main__":
    unittest.main()
