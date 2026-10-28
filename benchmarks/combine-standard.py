"""Combine unchanged peer cases with a complete rerun of revised Glory.

All 15 Glory cases are replaced together, regardless of relative speed. Earlier
results remain immutable. Every peer/harness input must still match its run.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("peers", type=Path)
parser.add_argument("glory", type=Path)
parser.add_argument("--out", type=Path, required=True)
args = parser.parse_args()
old = json.loads((args.peers / "inputs.json").read_text(encoding="utf-8"))
new = json.loads((args.glory / "inputs.json").read_text(encoding="utf-8"))
assert old.get("completed") and new.get("completed")
assert len(old["frameworks"]) == 8 and new["frameworks"] == ["glory"]
assert old["root"] == new["root"] and old["environment"] == new["environment"]
root, out = Path(new["root"]), args.out.resolve()
assert out.as_posix().lower().startswith("d:/buildcache/")
out.mkdir(parents=True, exist_ok=False)
inputs = {file: digest for file, digest in old["source_sha256"].items()
          if not file.startswith("frameworks/keyed/glory/")}
for file, digest in new["source_sha256"].items():
    if file in inputs:
        assert inputs[file] == digest, f"Peer/harness input changed between runs: {file}"
    inputs[file] = digest
for file, digest in inputs.items():
    assert hashlib.sha256((root / file).read_bytes()).hexdigest() == digest, file
    origin = args.glory if file in new["source_sha256"] else args.peers
    source = origin / "inputs" / file
    assert hashlib.sha256(source.read_bytes()).hexdigest() == digest, source
    target = out / "inputs" / file
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
(out / "results").mkdir()
for origin, keep_glory in [(args.peers, False), (args.glory, True)]:
    for file in (origin / "results").glob("*.json"):
        result = json.loads(file.read_text(encoding="utf-8"))
        if result["framework"].startswith("glory-v") == keep_glory:
            shutil.copy2(file, out / "results" / file.name)
assert len(list((out / "results").glob("*.json"))) == 120
metadata = {**new, "frameworks": old["frameworks"], "source_sha256": inputs,
            "completed": datetime.now(timezone.utc).isoformat(), "result_files": 120,
            "collection": "Staged: unchanged peers preserved; all 15 Glory cases rerun after generic runtime changes",
            "reference_runs": {"peers": str(args.peers.resolve()), "glory": str(args.glory.resolve())},
            "keyed_checks": [str(args.peers / "is-keyed.log"), str(args.glory / "is-keyed.log")],
            "retries": {"peers_run": old.get("retries", []), "glory_run": new.get("retries", [])}}
(out / "inputs.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
shutil.copy2(__file__, out / "combine-standard.py")
print(f"Combined 120 complete cases with verified source provenance: {out}")
