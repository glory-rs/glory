"""Resume only failed trace-collection cases from a preserved full standard run.

Each retry runs a whole case with upstream defaults; partial traces and logs
are archived first. Successful cases and slow samples are never replaced.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("run", type=Path)
args = parser.parse_args()
out = args.run.resolve()
metadata = json.loads((out / "inputs.json").read_text(encoding="utf-8"))
assert not metadata.get("completed"), "The run is already complete"
root = Path(metadata["root"])
driver = root / "webdriver-ts"
env = dict(os.environ, TEMP=str(out / "tmp"), TMP=str(out / "tmp"))
assert out.as_posix().lower().startswith("d:/buildcache/")
shutil.copy2(__file__, out / "finish-standard.py")
shutil.copytree(driver / "results", out / "initial-results")
initial_log = (out / "benchmark.log").read_text(encoding="utf-8")
assert "No commit event found" in initial_log, "Inspect the initial failure before resuming"
versions = {}
for file in (driver / "results").glob("*_01_run1k.json"):
    data = json.loads(file.read_text(encoding="utf-8"))
    for name in metadata["frameworks"]:
        if data["framework"] == name + "-keyed" or data["framework"].startswith(name + "-v"):
            versions[name] = data["framework"]
assert set(versions) == set(metadata["frameworks"])
cases = ["01_run1k", "02_replace1k", "03_update10th1k_x16", "04_select1k", "05_swap1k",
         "06_remove-one-1k", "07_create10k", "08_create1k-after1k_x2", "09_clear1k_x8",
         "21_ready-memory", "22_run-memory", "25_run-clear-memory", "41_size-uncompressed",
         "42_size-compressed", "43_first-paint"]
def valid(name, case):
    file = driver / "results" / f"{versions[name]}_{case}.json"
    if not file.exists():
        return False
    result = json.loads(file.read_text(encoding="utf-8"))
    expected = 25 if case.startswith("04_") else 15 if result["type"] == "cpu" else 1
    return all(len(value["values"]) == expected for value in result["values"].values())

attempts = []
for name in metadata["frameworks"]:
    groups = dict.fromkeys("40_sizes" if case.startswith(("41_", "42_", "43_")) else case
                           for case in cases if not valid(name, case))
    for case in groups:
        reason = r"Executing frameworks/keyed/" + re.escape(name) + r"(?:/[^ ]*)? and benchmark " + re.escape(case) + r" failed: No commit event found"
        assert re.search(reason, initial_log), f"Different initial failure; inspect {name} {case} before resuming"
        for attempt in range(1, 4):
            archive = out / "retries" / f"{name}-{case}-{attempt}"
            archive.mkdir(parents=True)
            prefix = versions[name] + "_" + case
            for file in (driver / "traces").glob(prefix + "*"):
                shutil.copy2(file, archive / file.name)
            with (archive / "benchmark.log").open("w", encoding="utf-8") as log:
                result = subprocess.run(["node", "dist/benchmarkRunner.js", "--headless",
                                         "--chromeBinary", metadata["chrome"], "--framework", "keyed/" + name,
                                         "--benchmark", case], cwd=driver, env=env, stdout=log,
                                        stderr=subprocess.STDOUT, check=False)
            attempts.append({"framework": name, "case": case, "attempt": attempt,
                             "exit_code": result.returncode, "archive": str(archive)})
            metadata["retries"] = attempts
            (out / "inputs.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
            if result.returncode == 0:
                break
            text = (archive / "benchmark.log").read_text(encoding="utf-8")
            assert "No commit event found" in text, f"Different failure; inspect {archive}"
        else:
            raise RuntimeError(f"Trace collection still failing: {name} {case}")
assert all(valid(name, case) for name in metadata["frameworks"] for case in cases)
for file, digest in metadata["source_sha256"].items():
    assert hashlib.sha256((root / file).read_bytes()).hexdigest() == digest, file
(out / "results").mkdir(exist_ok=True)
for source in (driver / "results").glob("*.json"):
    target = out / "results" / source.name
    if target.exists():
        assert source.read_bytes() == target.read_bytes(), f"Successful result changed: {source.name}"
    else:
        shutil.copy2(source, target)
metadata["completed"] = datetime.now(timezone.utc).isoformat()
metadata["result_files"] = len(list((out / "results").glob("*.json")))
assert metadata["result_files"] == len(metadata["frameworks"]) * 15
(out / "inputs.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
print(f"Completed {metadata['result_files']} result files; preserved {len(attempts)} whole-case collection retries")
