"""Summarize completed alternating runs, retaining every raw sample in evidence.json."""
import argparse
import json
import statistics
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("evidence", type=Path)
parser.add_argument("--out", required=True, type=Path)
args = parser.parse_args()
raw = json.loads(args.evidence.read_text(encoding="utf-8"))
assert raw.get("completed") and not raw.get("failure")
variants = list(raw["artifacts"])
runs = raw["rounds"]
rounds = len({run["round"] for run in runs})
assert rounds and len(runs) == rounds * len(variants)
assert all(sum(run["name"] == name for run in runs) == rounds for name in variants)
samples_per_run = raw["rounds"][0]["samples"]
sample_count = sum(len(item["samples"]) for run in runs for item in run["results"])
summary = {"schema": 1, "evidence": str(args.evidence.resolve()),
           "contract": raw["contract"], "profiles": raw["profiles"],
           "completed": raw["completed"], "browser": raw["browser"],
           "rounds": rounds, "samples_per_operation_variant": rounds*samples_per_run, "samples_total": sample_count,
           "statistic": "median of all retained samples; paired directions compare per-round medians",
           "baseline_snapshot": raw["baseline_source"]["snapshot"],
           "candidate_snapshot": raw["source"]["snapshot"],
           "artifacts": raw["artifacts"], "operations": {},
           "acceptance": raw["acceptance"]}
for key in [item["key"] for item in runs[0]["results"]]:
    result = {}
    for name in variants:
        matching = [next(item for item in run["results"] if item["key"] == key)
                    for run in runs if run["name"] == name]
        assert all(len(item["samples"]) == 12 for item in matching)
        samples = [sample for item in matching for sample in item["samples"]]
        metrics = {"wall_ms": lambda sample: sample["elapsed_ms"],
                   **{field: lambda sample, field=field: sample["metrics"][field]
                      for field in ["ScriptDuration", "LayoutDuration", "RecalcStyleDuration", "TaskDuration"]}}
        result[name] = {field: {"median": statistics.median(map(read, samples)),
                               "min": min(map(read, samples)), "max": max(map(read, samples)),
                               "round_medians": [statistics.median(map(read, item["samples"])) for item in matching]}
                        for field, read in metrics.items()}
    for name in [variant for variant in variants if variant not in ("before","dioxus","leptos")]:
        result[name]["versus_before_percent"] = {
            field: (100 * (result[name][field]["median"] / result["before"][field]["median"] - 1)
                    if result["before"][field]["median"] else None)
            for field in result["before"]}
        result[name]["faster_rounds"] = {
            field: sum(candidate < baseline for candidate, baseline in zip(
                result[name][field]["round_medians"], result["before"][field]["round_medians"]))
            for field in result["before"]}
    summary["operations"][key] = result
args.out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
for key, result in summary["operations"].items():
    values = [f'{result[name]["ScriptDuration"]["median"]:.3f}' for name in variants]
    deltas = [f'{result[name]["versus_before_percent"]["ScriptDuration"]:+.1f}%' for name in variants if "versus_before_percent" in result[name]]
    print(" | ".join([key, *values, *deltas]))
