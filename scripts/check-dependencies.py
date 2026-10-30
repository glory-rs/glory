"""Audit maintained runtime, tooling and application lockfiles with RustSec."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
# Peer comparison fixtures are frozen measurement inputs, not shipped apps.
LOCKFILES = [
    "Cargo.lock", "tools/cli/Cargo.lock",
    "examples/counter/Cargo.lock", "examples/display-web/Cargo.lock",
    "examples/mobile/Cargo.lock", "examples/routing/Cargo.lock",
    "examples/semantic-web/Cargo.lock", "benchmarks/glory/Cargo.lock",
    "benchmarks/role-patches/Cargo.lock", "benchmarks/wasm-size/Cargo.lock",
    "benchmarks/scenarios/glory/Cargo.lock",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit-bin", help="Pinned cargo-audit executable; otherwise use cargo audit")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--no-fetch", action="store_true")
    args = parser.parse_args()
    cargo_home = Path(os.environ["CARGO_HOME"]).resolve()
    database = cargo_home / "advisory-db"
    args.out.mkdir(parents=True, exist_ok=True)
    command = [args.audit_bin, "audit"] if args.audit_bin else ["cargo", "audit"]
    summaries = []
    failed = False
    for index, relative in enumerate(LOCKFILES):
        flags = ["--no-fetch"] if args.no_fetch or index else []
        result = subprocess.run(
            [*command, "--db", str(database), "--file", str(ROOT / relative),
             "--deny", "unsound", "--deny", "yanked", "--json", *flags],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
        stem = relative.replace("/", "__").replace(".lock", "")
        (args.out / f"{stem}.json").write_text(result.stdout, encoding="utf-8")
        (args.out / f"{stem}.log").write_text(result.stderr, encoding="utf-8")
        try:
            report = json.loads(result.stdout)
            if not isinstance(report.get("vulnerabilities"), dict) or not report.get("database"):
                raise ValueError("missing audit result")
            warnings = {
                kind: [f"{item['package']['name']} {item['package']['version']} "
                       f"{(item.get('advisory') or {}).get('id', '')}" for item in items]
                for kind, items in (report.get("warnings") or {}).items()
            }
            summary = {"lockfile": relative, "exit_code": result.returncode,
                       "database": report.get("database"),
                       "vulnerabilities": report.get("vulnerabilities"), "warnings": warnings}
        except (ValueError, KeyError, TypeError):
            summary = {"lockfile": relative, "exit_code": result.returncode,
                       "error": "cargo-audit did not return a valid report"}
            failed = True
        summaries.append(summary)
        failed |= result.returncode != 0
        print(f"{relative}: {'PASS' if result.returncode == 0 else 'FAIL'}", flush=True)
        for kind, items in summary.get("warnings", {}).items():
            for item in items:
                print(f"  {kind}: {item}", flush=True)
    (args.out / "summary.json").write_text(
        json.dumps(summaries, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
