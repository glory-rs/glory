"""The known glib warning must never hide another finding or auditor failure."""
from contextlib import redirect_stdout
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "dependency_audit", Path(__file__).with_name("check-dependencies.py"))
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


def finding(kind="unsound", package="glib", advisory="RUSTSEC-2024-0429"):
    return {"kind": kind, "package": {"name": package, "version": "0.18.5"},
            "advisory": {"id": advisory, "package": package, "informational": kind}}


def report():
    return {"database": {"advisory-count": 1}, "lockfile": {"dependency-count": 1},
            "settings": {"ignore": [], "target_arch": [], "target_os": [], "severity": None,
                         "informational_warnings": ["unmaintained", "unsound", "notice"]},
            "vulnerabilities": {"found": False, "count": 0, "list": []}, "warnings": {}}


def known_report():
    value = report()
    value["warnings"]["unsound"] = [finding()]
    return value


class AuditPolicyTests(unittest.TestCase):
    def test_clean_and_existing_maintenance_warnings_still_pass(self):
        value = report()
        self.assertEqual(AUDIT.classify_report(value, 0)["status"], "PASS")
        for kind in ("unmaintained", "notice"):
            value["warnings"][kind] = [finding(kind)]
        self.assertEqual(AUDIT.classify_report(value, 0)["exit_code"], 0)

    def test_only_known_glib_unsoundness_is_warning_only(self):
        value = known_report()
        second = finding()
        second["package"]["version"] = "0.18.5"
        value["warnings"]["unsound"].append(second)
        result = AUDIT.classify_report(value, 0)
        self.assertEqual(result["status"], "WARN")
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(len(result["known_glib_warnings"]), 2)

    def test_nonzero_tool_exit_is_never_downgraded(self):
        for code in (1, 2, 101, -9):
            with self.subTest(code=code):
                result = AUDIT.classify_report(known_report(), code)
                self.assertEqual(result["exit_code"], 1)
                self.assertEqual(result["audit_exit_code"], code)

    def test_auditor_diagnostics_are_not_hidden_by_a_valid_known_warning(self):
        for stderr in ("could not query the registry index", "\n"):
            with self.subTest(stderr=stderr):
                result = AUDIT.classify_report(known_report(), 0, stderr)
                self.assertEqual(result["exit_code"], 1)

    def test_new_glib_advisory_or_same_id_for_another_package_blocks(self):
        for package, advisory in (("glib-sys", "RUSTSEC-2024-0429"),
                                  ("glib", "RUSTSEC-2099-0001")):
            with self.subTest(package=package, advisory=advisory):
                value = report()
                value["warnings"]["unsound"] = [finding(package=package, advisory=advisory)]
                self.assertEqual(AUDIT.classify_report(value, 0)["exit_code"], 1)

    def test_inconsistent_advisory_identity_does_not_get_exception(self):
        for key, value in (("package", "other"), ("informational", "notice")):
            with self.subTest(key=key):
                item = known_report()
                item["warnings"]["unsound"][0]["advisory"][key] = value
                with self.assertRaises(ValueError):
                    AUDIT.classify_report(item, 0)

    def test_other_glib_versions_are_not_exempt(self):
        for version in ("0.18.4", "0.19.0", "0.18.5%\n::error::injected"):
            with self.subTest(version=version):
                value = known_report()
                value["warnings"]["unsound"][0]["package"]["version"] = version
                self.assertEqual(AUDIT.classify_report(value, 0)["exit_code"], 1)

    def test_notice_and_maintenance_cannot_hide_an_unsound_advisory(self):
        for kind in ("notice", "unmaintained"):
            for advisory in (None, [], {}, finding()["advisory"]):
                with self.subTest(kind=kind, advisory=advisory):
                    value = report()
                    value["warnings"][kind] = [{**finding(kind), "advisory": advisory}]
                    with self.assertRaises(ValueError):
                        AUDIT.classify_report(value, 0)

    def test_mixed_findings_block_even_if_tool_incorrectly_returns_zero(self):
        for kind in ("unsound", "yanked", "unknown"):
            with self.subTest(kind=kind):
                value = known_report()
                value["warnings"].setdefault(kind, []).append(
                    finding(kind, "other", "RUSTSEC-2099-0001"))
                self.assertEqual(AUDIT.classify_report(value, 0)["exit_code"], 1)

    def test_known_id_in_vulnerability_section_is_still_fatal(self):
        value = known_report()
        value["vulnerabilities"] = {"found": True, "count": 1, "list": [finding()]}
        self.assertEqual(AUDIT.classify_report(value, 0)["exit_code"], 1)

    def test_empty_missing_or_inconsistent_report_sections_are_rejected(self):
        variants = [None, [], {}, {**report(), "error": "database failure"},
                    {**known_report(), "failure": {"reason": "partial scan"}}]
        for section in ("database", "lockfile", "settings", "vulnerabilities", "warnings"):
            value = report()
            del value[section]
            variants.append(value)
        value = report()
        del value["settings"]["severity"]
        variants.append(value)
        for count, found, items in ((True, False, []), (1, False, []),
                                    (0, True, []), (0, False, None)):
            value = report()
            value["vulnerabilities"] = {"count": count, "found": found, "list": items}
            variants.append(value)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    AUDIT.classify_report(value, 0)

    def test_hidden_advisories_and_filtered_audits_are_rejected(self):
        for key, setting in (("ignore", ["RUSTSEC-2024-0429"]),
                             ("target_arch", ["x86_64"]), ("target_os", ["windows"]),
                             ("severity", 7.0),
                             ("informational_warnings", ["unmaintained", "notice"])):
            with self.subTest(key=key):
                value = known_report()
                value["settings"][key] = setting
                with self.assertRaises(ValueError):
                    AUDIT.classify_report(value, 0)

    def test_empty_database_and_invalid_warning_shapes_are_rejected(self):
        variants = []
        for count in (0, True):
            value = report()
            value["database"]["advisory-count"] = count
            variants.append(value)
        for item in (None, "glib", {**finding(), "kind": "notice"},
                     {**finding(), "package": None}):
            value = report()
            value["warnings"]["unsound"] = [item]
            variants.append(value)
        for value in variants:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    AUDIT.classify_report(value, 0)


class AuditCommandTests(unittest.TestCase):
    def invoke(self, responses):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as directory:
            root = Path(directory)
            if os.name == "nt":
                self.assertTrue(root.resolve().is_relative_to(Path("D:/BuildCache").resolve()))
            locks = [f"case-{index}/Cargo.lock" for index in range(len(responses))]
            for relative in locks:
                lock = root / relative
                lock.parent.mkdir()
                lock.write_bytes(b"version = 4\n")
            out = root / "results"
            output = io.StringIO()
            with patch.object(AUDIT, "ROOT", root), patch.object(AUDIT, "LOCKFILES", locks), \
                    patch.object(AUDIT, "ensure_library_lockfiles"), \
                    patch.object(AUDIT.subprocess, "run", side_effect=responses) as run, \
                    patch.dict(os.environ, {"CARGO_HOME": str(root / "cargo")}), \
                    patch.object(sys, "argv", ["check-dependencies.py", "--out", str(out)]), \
                    redirect_stdout(output):
                result = AUDIT.main()
            summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
            snapshots = [(out / "lockfiles" / relative).read_bytes() for relative in locks]
            raw = [(out / (relative.replace("/", "__").replace(".lock", "") + ".json")).read_text(
                encoding="utf-8") if not isinstance(response, OSError) else None
                   for relative, response in zip(locks, responses)]
            return result, summary, output.getvalue(), run.call_args_list, snapshots, raw

    def response(self, value, code=0):
        return subprocess.CompletedProcess([], code, json.dumps(value), "")

    def test_known_warning_is_visible_and_raw_report_is_preserved(self):
        value = known_report()
        result, summary, output, calls, snapshots, raw = self.invoke([self.response(value)])
        self.assertEqual(result, 0)
        self.assertEqual(summary[0]["status"], "WARN")
        self.assertIn("::warning file=case-0/Cargo.lock", output)
        self.assertIn("RUSTSEC-2024-0429", output)
        self.assertEqual(json.loads(raw[0]), value)
        self.assertEqual(snapshots, [b"version = 4\n"])
        command = calls[0].args[0]
        self.assertIn("yanked", command)
        self.assertNotIn("unsound", command)
        self.assertNotIn("--ignore", command)

    def test_later_warning_cannot_erase_an_earlier_lock_failure(self):
        value = report()
        value["vulnerabilities"] = {"found": True, "count": 1, "list": [finding()]}
        result, summary, _, calls, _, _ = self.invoke([
            self.response(value, 1), self.response(known_report())])
        self.assertEqual(result, 1)
        self.assertEqual([x["status"] for x in summary], ["FAIL", "WARN"])
        self.assertNotIn("--no-fetch", calls[0].args[0])
        self.assertIn("--no-fetch", calls[1].args[0])

    def test_tool_failure_valid_json_and_known_warning_still_fail(self):
        result, summary, _, _, _, _ = self.invoke([self.response(known_report(), 1)])
        self.assertEqual(result, 1)
        self.assertEqual(summary[0]["audit_exit_code"], 1)

    def test_registry_diagnostics_with_zero_exit_still_block(self):
        response = self.response(known_report())
        response.stderr = "error: could not read package registry entry"
        result, summary, _, _, _, _ = self.invoke([response])
        self.assertEqual(result, 1)
        self.assertEqual(summary[0]["audit_exit_code"], 0)

    def test_invalid_json_and_tool_start_error_still_fail(self):
        for response in (subprocess.CompletedProcess([], 0, "", ""),
                         subprocess.CompletedProcess([], 0, "{", ""),
                         FileNotFoundError("auditor missing")):
            with self.subTest(response=response):
                result, summary, _, _, _, _ = self.invoke([response])
                self.assertEqual(result, 1)
                self.assertEqual(summary[0]["status"], "FAIL")

    def test_duplicate_report_fields_are_an_audit_error(self):
        raw = json.dumps(report())[:-1] + ',"vulnerabilities":{"found":false,"count":0,"list":[]}}'
        result, _, _, _, _, _ = self.invoke([subprocess.CompletedProcess([], 0, raw, "")])
        self.assertEqual(result, 1)

    def test_annotation_and_plain_log_messages_cannot_inject_commands(self):
        value = known_report()
        value["warnings"]["unsound"][0]["package"]["version"] = "0.18.5%\n::error::injected"
        result, _, output, _, _, _ = self.invoke([self.response(value)])
        self.assertEqual(result, 1)
        self.assertNotIn("\n::error::", output)
        self.assertIn("%25%0A", output)
        self.assertEqual(AUDIT.annotation("x,y:z\r\n", True), "x%2Cy%3Az%0D%0A")


if __name__ == "__main__":
    unittest.main()
