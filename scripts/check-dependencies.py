"""Audit maintained runtime, tooling and application lockfiles with RustSec."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import shutil
from library_lockfiles import LIBRARY_MANIFESTS, ensure_library_lockfiles

ROOT = Path(__file__).resolve().parents[1]
# Peer comparison fixtures are frozen measurement inputs, not shipped apps.
LOCKFILES = [str(Path(manifest).with_name("Cargo.lock")).replace("\\", "/") for manifest in LIBRARY_MANIFESTS]


def classify_report(report, audit_exit_code, audit_stderr=""):
    """Keep tool failures fatal and allow only the known GTK3 glib warning."""
    if not isinstance(report, dict) or set(report) != {
            "database", "lockfile", "settings", "vulnerabilities", "warnings"}:
        raise ValueError("missing audit report sections or unexpected auditor fields")
    database = report.get("database")
    lockfile = report.get("lockfile")
    settings = report.get("settings")
    vulnerabilities = report.get("vulnerabilities")
    warnings = report.get("warnings")
    if not all(isinstance(value, dict) for value in
               (database, lockfile, settings, vulnerabilities, warnings)):
        raise ValueError("missing audit report sections")
    for section, key in ((database, "advisory-count"), (lockfile, "dependency-count")):
        if type(section.get(key)) is not int or section[key] <= 0:
            raise ValueError(f"invalid {key}")
    if any(settings.get(key) != [] for key in ("ignore", "target_arch", "target_os")):
        raise ValueError("unexpected advisory exclusions or target filters")
    if "severity" not in settings or settings["severity"] is not None:
        raise ValueError("unexpected severity filter")
    informational = settings.get("informational_warnings")
    if (not isinstance(informational, list) or not all(isinstance(x, str) for x in informational) or
            not {"unmaintained", "unsound", "notice"}.issubset(informational)):
        raise ValueError("informational warnings were filtered")
    found = vulnerabilities.get("found")
    count = vulnerabilities.get("count")
    items = vulnerabilities.get("list")
    if (type(found) is not bool or type(count) is not int or not isinstance(items, list)
            or count != len(items) or found != bool(items)):
        raise ValueError("inconsistent vulnerability result")

    reasons = []
    known_glib = []
    if audit_exit_code != 0:
        reasons.append(f"auditor exited with {audit_exit_code}")
    if audit_stderr:
        reasons.append("auditor reported unexpected diagnostics on stderr")
    if found:
        reasons.append("security vulnerabilities")
    for kind, entries in warnings.items():
        if not isinstance(entries, list):
            raise ValueError("invalid warning list")
        for item in entries:
            if not isinstance(item, dict) or item.get("kind") != kind:
                raise ValueError("inconsistent warning kind")
            package = item.get("package")
            if (not isinstance(package, dict) or
                    not all(isinstance(package.get(key), str) and package[key]
                            for key in ("name", "version"))):
                raise ValueError("invalid warning package")
            advisory = item.get("advisory")
            if kind in ("unsound", "unmaintained", "notice"):
                if (not isinstance(advisory, dict)
                        or not isinstance(advisory.get("id"), str) or not advisory["id"]
                        or advisory.get("package") != package["name"]
                        or advisory.get("informational") != kind):
                    raise ValueError("invalid or inconsistent warning advisory")
            elif advisory is not None and not isinstance(advisory, dict):
                raise ValueError("invalid warning advisory")
            # GTK3 bindings retain this upstream issue. Keep it visible without
            # exempting other glib advisories or moving it out of the report.
            known = (kind == "unsound" and package["name"] == "glib" and package["version"] == "0.18.5"
                     and advisory.get("id") == "RUSTSEC-2024-0429"
                     and advisory.get("package") == "glib"
                     and advisory.get("informational") == "unsound")
            if known:
                known_glib.append(item)
            elif kind not in ("unmaintained", "notice"):
                reasons.append(f"blocking {kind} warning")
    return {
        "exit_code": int(bool(reasons)),
        "audit_exit_code": audit_exit_code,
        "status": "FAIL" if reasons else "WARN" if known_glib else "PASS",
        "blocking_reasons": reasons,
        "known_glib_warnings": known_glib,
    }


def annotation(value, property_value=False):
    value = str(value).replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return value.replace(",", "%2C").replace(":", "%3A") if property_value else value


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate report field: {key}")
        result[key] = value
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit-bin", help="Pinned cargo-audit executable; otherwise use cargo audit")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--no-fetch", action="store_true")
    args = parser.parse_args()
    ensure_library_lockfiles()
    cargo_home = Path(os.environ["CARGO_HOME"]).resolve()
    database = cargo_home / "advisory-db"
    args.out.mkdir(parents=True, exist_ok=True)
    command = [args.audit_bin, "audit"] if args.audit_bin else ["cargo", "audit"]
    summaries = []
    failed = False
    for index, relative in enumerate(LOCKFILES):
        # Preserve the exact generated library graph alongside the audit result.
        snapshot = args.out / "lockfiles" / relative
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, snapshot)
        flags = ["--no-fetch"] if args.no_fetch or index else []
        stem = relative.replace("/", "__").replace(".lock", "")
        # Check every unsound warning below instead of asking cargo-audit to
        # deny the whole class. A nonzero tool exit is never downgraded.
        try:
            result = subprocess.run(
                [*command, "--db", str(database), "--file", str(ROOT / relative),
                 "--deny", "yanked", "--json", *flags],
                cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="strict",
            )
        except (OSError, UnicodeError) as error:
            (args.out / f"{stem}.json").write_text("", encoding="utf-8")
            (args.out / f"{stem}.log").write_text(str(error), encoding="utf-8")
            summaries.append({"lockfile": relative, "exit_code": 1,
                              "audit_exit_code": None, "status": "FAIL", "error": str(error)})
            failed = True
            print(f"{relative}: FAIL (auditor could not run)", flush=True)
            continue
        (args.out / f"{stem}.json").write_text(result.stdout, encoding="utf-8")
        (args.out / f"{stem}.log").write_text(result.stderr, encoding="utf-8")
        try:
            report = json.loads(result.stdout, object_pairs_hook=unique_fields)
            policy = classify_report(report, result.returncode, result.stderr)
            warnings = {
                kind: [f"{item['package']['name']} {item['package']['version']} "
                       f"{(item.get('advisory') or {}).get('id', '')}" for item in items]
                for kind, items in (report.get("warnings") or {}).items()
            }
            summary = {"lockfile": relative, **policy,
                       "database": report.get("database"),
                       "vulnerabilities": report.get("vulnerabilities"), "warnings": warnings}
        except (ValueError, KeyError, TypeError) as error:
            summary = {"lockfile": relative, "exit_code": 1,
                       "audit_exit_code": result.returncode, "status": "FAIL",
                       "error": f"cargo-audit did not return a valid report: {error}"}
            failed = True
        summaries.append(summary)
        failed |= summary["exit_code"] != 0
        print(f"{relative}: {summary['status']}", flush=True)
        for item in summary.get("known_glib_warnings", []):
            message = (f"RUSTSEC-2024-0429: glib {item['package']['version']} remains in the "
                       "GTK3 dependency chain; this known version/advisory is warning-only. "
                       "Other vulnerabilities, unsoundness, yanked packages and audit errors remain fatal.")
            print(f"::warning file={annotation(relative, True)},title=Known glib advisory::"
                  f"{annotation(message)}", flush=True)
        for kind, items in summary.get("warnings", {}).items():
            for item in items:
                print(f"  {annotation(kind)}: {annotation(item)}", flush=True)
    (args.out / "summary.json").write_text(
        json.dumps(summaries, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
