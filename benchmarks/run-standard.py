"""Freeze and run the sibling js-framework-benchmark's enabled default suite.

Build first with scripts/build-js-framework-benchmark.ps1 and peer npm build-prod
commands. The upstream server must already be serving port 8080. This runner
does not change upstream timings, throttles, samples, or benchmark definitions.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from datetime import datetime, timezone

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2] / "js-framework-benchmark")
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--chrome", default="C:/Program Files/Google/Chrome/Application/chrome.exe")
parser.add_argument("--frameworks", nargs="+", choices=["glory", "leptos", "dioxus", "react-hooks", "solid", "vue", "svelte", "vanillajs"],
                    default=["glory", "leptos", "dioxus", "react-hooks", "solid", "vue", "svelte", "vanillajs"])
parser.add_argument("--glory-inputs", default="D:/BuildCache/glory-display-completion-20261028/verified/inputs/inputs.json")
args = parser.parse_args()
root, out = args.root.resolve(), args.out.resolve()
assert out.as_posix().lower().startswith("d:/buildcache/")
out.mkdir(parents=True, exist_ok=False)
frameworks = args.frameworks
env = dict(os.environ, TEMP=str(out / "tmp"), TMP=str(out / "tmp"))
(out / "tmp").mkdir()
# The current checkout has only keyed implementations; the upstream inventory
# enumerates both directories even when every requested implementation is keyed.
(root / "frameworks/non-keyed").mkdir(exist_ok=True)
driver = root / "webdriver-ts"
existing = list((driver / "results").glob("*.json"))
assert not existing, "Preserve existing results and use a clean result directory before running"

inputs = {}
for folder in [*(root / "frameworks/keyed" / name for name in frameworks),
               root / "webdriver-ts/src", root / "webdriver-ts/dist", root / "server/src",
               root / "css", root / "fonts"]:
    for source in folder.rglob("*"):
        if not source.is_file() or any(part in {"node_modules", "target", ".git"} for part in source.relative_to(folder).parts):
            continue
        relative = source.relative_to(root)
        target = out / "inputs" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        inputs[relative.as_posix()] = hashlib.sha256(source.read_bytes()).hexdigest()
for package in ["webdriver-ts", "server"]:
    for name in ["package.json", "package-lock.json", "tsconfig.json"]:
        source = root / package / name
        if source.is_file():
            relative = source.relative_to(root)
            target = out / "inputs" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            inputs[relative.as_posix()] = hashlib.sha256(source.read_bytes()).hexdigest()
shutil.copy2(__file__, out / "run-standard.py")
metadata = {"started": datetime.now(timezone.utc).isoformat(), "root": str(root),
            "frameworks": frameworks, "source_sha256": inputs, "chrome": args.chrome,
            "configuration": "Unmodified upstream defaults: 15 CPU samples (selection adds 10), one per memory/size case; default CPU throttles; no slowest samples dropped",
            "glory_inputs": args.glory_inputs}
metadata["upstream_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
metadata["environment"] = json.loads(subprocess.check_output([
    "powershell", "-NoProfile", "-Command",
    "[pscustomobject]@{cpu=(Get-CimInstance Win32_Processor).Name;os=(Get-CimInstance Win32_OperatingSystem).Caption;chrome=(Get-Item -LiteralPath '" + args.chrome + "').VersionInfo.ProductVersion;node=(node --version);rust=(rustc --version)} | ConvertTo-Json -Compress"
], text=True, encoding="utf-8"))
(out / "inputs.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
common = ["--headless", "--chromeBinary", args.chrome, "--framework", *("keyed/" + f for f in frameworks)]
for script, log in [("isKeyed.js", "is-keyed.log"), ("benchmarkRunner.js", "benchmark.log")]:
    with (out / log).open("w", encoding="utf-8") as output:
        result = subprocess.run(["node", "dist/" + script, *common], cwd=driver, env=env,
                                stdout=output, stderr=subprocess.STDOUT, check=False)
    assert result.returncode == 0, f"{script} failed; see {out / log}"
    if script == "isKeyed.js":
        text = (out / log).read_text(encoding="utf-8")
        assert text.count("It'll appear as keyed in the results") == len(frameworks), text[-4000:]
shutil.copytree(driver / "results", out / "results")
results = [json.loads(file.read_text(encoding="utf-8")) for file in (out / "results").glob("*.json")]
assert len(results) == len(frameworks) * 15, f"Expected {len(frameworks) * 15} result files, got {len(results)}"
for result in results:
    for values in result["values"].values():
        assert values["values"], result
        expected = 25 if result["benchmark"].startswith("04_") else 15 if result["type"] == "cpu" else 1
        assert len(values["values"]) == expected, result
for file, digest in inputs.items():
    assert hashlib.sha256((root / file).read_bytes()).hexdigest() == digest, f"Input changed during run: {file}"
metadata["completed"] = datetime.now(timezone.utc).isoformat()
metadata["result_files"] = len(results)
(out / "inputs.json").write_text(json.dumps(metadata, indent=2), encoding="utf-8")
print(f"Completed {len(frameworks)} keyed implementations, {len(results)} complete result files: {out}")
