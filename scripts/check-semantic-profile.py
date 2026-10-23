"""Measure the same native list workload with and without optional profiling.

Build the profile example separately, with Cargo target/cache/temp on D:. This
driver stores every observation, including slower profiled runs. It measures
offscreen CPU work; neither phase sums nor this benchmark establish display FPS.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys


def build_path(value):
    path = Path(value).resolve()
    portable = path.as_posix().lower()
    if not portable.startswith(("d:/buildcache/", "/mnt/d/buildcache/")):
        raise argparse.ArgumentTypeError("must resolve under D:/BuildCache")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=build_path, required=True)
    parser.add_argument("--out", type=build_path, required=True)
    parser.add_argument("--pairs", type=int, default=4)
    parser.add_argument("--quick", action="store_true")
    args = parser.parse_args()
    if not 2 <= args.pairs <= 20:
        parser.error("--pairs must be between 2 and 20")
    if not args.binary.is_file():
        parser.error("--binary must be an existing compiled profile example")
    args.out.mkdir(parents=True, exist_ok=False)
    groups = {}
    observations = []
    for pair in range(args.pairs):
        for enabled in ([True, False] if pair % 2 == 0 else [False, True]):
            command = [str(args.binary)]
            if args.quick:
                command.append("--quick")
            if not enabled:
                command.append("--no-profile")
            label = f"{pair}-{'on' if enabled else 'off'}"
            # Save stdout/stderr even on failure, before accepting any sample.
            with (args.out / f"{label}.jsonl").open("w", encoding="utf-8") as stdout:
                with (args.out / f"{label}.stderr").open("w", encoding="utf-8") as stderr:
                    subprocess.run(command, stdout=stdout, stderr=stderr, check=True)
            records = [json.loads(line) for line in
                       (args.out / f"{label}.jsonl").read_text(encoding="utf-8").splitlines()]
            runs = [record for record in records if record["type"] == "run"]
            assert len(runs) == (8 if args.quick else 24), len(runs)
            for run in runs:
                assert run["profile"] == enabled
                identity = tuple(run[name] for name in
                                 ("rows", "measured", "repetition", "pipeline"))
                phases = [record for record in records if record["type"] == "phase"
                          and record["interval"] == "steady"
                          and tuple(record[name] for name in
                                    ("rows", "measured", "repetition", "pipeline")) == identity]
                for phase in phases:
                    assert phase["calls"] > 0
                    assert 0 <= phase["max_ns"] <= phase["inclusive_ns"]
                    assert phase["invalid_clock"] == 0
                if not enabled:
                    assert not phases
                observation = dict(run, pair=pair, phases=phases)
                observations.append(observation)
                key = (run["rows"], run["measured"], run["pipeline"])
                groups.setdefault(key, {True: [], False: []})[enabled].append(observation)
            print(f"accepted {label}: {len(runs)} workloads", flush=True)
    summaries = []
    for (rows, measured, pipeline), variants in sorted(groups.items()):
        # Instrumentation must not change work performed or mounted budgets.
        for field in ("frames", "key_reads", "max_nodes", "max_graph", "max_engine", "buffer_bytes"):
            assert {run[field] for run in variants[True]} == {
                run[field] for run in variants[False]}, (rows, measured, pipeline, field)
        timing = {enabled: statistics.median(run["frame_ns"] / run["frames"] / 1e6
                                            for run in runs)
                  for enabled, runs in variants.items()}
        phase_values = {}
        for run in variants[True]:
            for phase in run["phases"]:
                phase_values.setdefault(phase["phase"], []).append(
                    phase["inclusive_ns"] / run["frames"] / 1e6)
        summaries.append({
            "rows": rows, "measured": measured, "pipeline": pipeline,
            "samples_per_variant": len(variants[True]),
            "off_median_ms_per_update": timing[False],
            "on_median_ms_per_update": timing[True],
            "on_over_off": timing[True] / timing[False],
            "inclusive_phase_median_ms_per_update": {
                phase: statistics.median(values) for phase, values in phase_values.items()},
            "key_reads": sorted({run["key_reads"] for run in variants[True]}),
            "max_nodes": max(run["max_nodes"] for run in variants[True]),
            "max_graph": max(run["max_graph"] for run in variants[True]),
            "max_engine": max(run["max_engine"] for run in variants[True]),
        })
    result = {
        "schema": 1, "binary": str(args.binary),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "platform": sys.platform, "pairs": args.pairs, "quick": args.quick,
        "scope": "offscreen CPU; inclusive phases overlap; no FPS/RSS or pre-change comparison",
        "cargo_target_dir": os.environ.get("CARGO_TARGET_DIR"),
        "summaries": summaries, "observations": observations,
    }
    (args.out / "evidence.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summaries, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
