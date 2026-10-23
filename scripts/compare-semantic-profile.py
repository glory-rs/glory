"""Compare preserved native list binaries with identical, verified workloads.

Build binaries first with the same toolchain/release settings and benchmark
source. Runs alternate before/after and profiling on/off. All raw observations
are retained; image hashes, actual row heights and mounted budgets must agree.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess


def build_path(value):
    path = Path(value).resolve()
    if not path.as_posix().lower().startswith(("d:/buildcache/", "/mnt/d/buildcache/")):
        raise argparse.ArgumentTypeError("must resolve under D:/BuildCache")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("before", "after", "out"):
        parser.add_argument("--" + name, type=build_path, required=True)
    parser.add_argument("--intermediate", type=build_path)
    parser.add_argument("--pairs", type=int, default=4)
    parser.add_argument("--quick", action="store_true")
    parser.add_argument("--scroll", action="store_true")
    parser.add_argument("--measure-limit", type=int, default=64)
    args = parser.parse_args()
    if not 2 <= args.pairs <= 20:
        parser.error("--pairs must be between 2 and 20")
    if args.measure_limit < 1:
        parser.error("--measure-limit must be positive")
    binaries = {"before": args.before, "after": args.after}
    if args.intermediate:
        binaries["scroll_only"] = args.intermediate
    for binary in binaries.values():
        if not binary.is_file():
            parser.error(f"missing compiled binary: {binary}")
    args.out.mkdir(parents=True, exist_ok=False)
    observations = []
    order = [(name, enabled) for enabled in [False, True] for name in binaries]
    for pair in range(args.pairs):
        current = order if pair % 2 == 0 else list(reversed(order))
        for name, enabled in current:
            label = f"{pair}-{name}-{'on' if enabled else 'off'}"
            command = [str(binaries[name]), "--measure-limit", str(args.measure_limit)]
            if args.quick:
                command.append("--quick")
            if args.scroll:
                command.append("--scroll")
            if not enabled:
                command.append("--no-profile")
            with (args.out / f"{label}.jsonl").open("w", encoding="utf-8") as out:
                with (args.out / f"{label}.stderr").open("w", encoding="utf-8") as err:
                    subprocess.run(command, stdout=out, stderr=err, check=True)
            records = [json.loads(line) for line in
                       (args.out / f"{label}.jsonl").read_text(encoding="utf-8").splitlines()]
            runs = [record for record in records if record["type"] == "run"]
            assert len(runs) == (8 if args.quick else 24)
            fields = ("rows", "measured", "repetition", "pipeline")
            for run in runs:
                assert run["profile"] == enabled and run["verified_heights"]
                assert run["measure_limit"] == args.measure_limit
                if run["measured"]:
                    assert run["max_measured"] >= args.measure_limit
                    if args.measure_limit >= 20:  # configured mounted-row limit
                        assert run["max_measured"] == args.measure_limit
                    assert run["precondition_steps"] > 0
                identity = tuple(run[field] for field in fields)
                phases = [record for record in records if record["type"] == "phase"
                          and record["interval"] == "steady"
                          and tuple(record[field] for field in fields) == identity]
                if not enabled:
                    assert not phases
                for phase in phases:
                    assert phase["calls"] > 0 and phase["invalid_clock"] == 0
                    assert 0 <= phase["max_ns"] <= phase["inclusive_ns"]
                observations.append(dict(run, variant=name, pair=pair, phases=phases))
            print(f"accepted {label}: {len(runs)} workloads", flush=True)
    # Compare each repeat, not merely sets or a representative final frame.
    canonical = {}
    visual = {}
    for run in observations:
        # A separate process/repetition/profile flag must reproduce positions,
        # boxes and pixels too, not just a lucky matched pair.
        identity = tuple(run[field] for field in ("rows", "measured", "pipeline"))
        outcome = tuple(run[field] for field in
                        ("frames", "pixel_hash", "position_hash", "geometry_hash",
                         "final_physical", "final_logical", "max_measured",
                         "max_nodes", "max_graph", "max_engine", "buffer_bytes"))
        assert canonical.setdefault(identity, outcome) == outcome, (identity, run["variant"], run["profile"])
        picture = tuple(run[field] for field in
                        ("frames", "pixel_hash", "position_hash", "geometry_hash",
                         "final_physical", "final_logical"))
        assert visual.setdefault((run["rows"], run["measured"]), picture) == picture
    groups = {}
    for run in observations:
        key = tuple(run[field] for field in ("rows", "measured", "pipeline", "profile"))
        groups.setdefault(key, {}).setdefault(run["variant"], []).append(run)
    comparisons = []
    for (rows, measured, pipeline, enabled), variants in sorted(groups.items()):
        summaries = {}
        for name, runs in variants.items():
            phases = {}
            for run in runs:
                for phase in run["phases"]:
                    phases.setdefault(phase["phase"], []).append(phase)
            summaries[name] = {
                "samples": len(runs),
                "median_ms_per_update": statistics.median(run["frame_ns"] / run["frames"] / 1e6 for run in runs),
                "layout_passes": sorted({run["layout_passes"] for run in runs}),
                "key_reads": sorted({run["key_reads"] for run in runs}),
                "max_nodes": max(run["max_nodes"] for run in runs),
                "max_graph": max(run["max_graph"] for run in runs),
                "max_engine": max(run["max_engine"] for run in runs),
                "phase_calls": {phase: sorted({value["calls"] for value in values}) for phase, values in phases.items()},
                "inclusive_phase_median_ms": {phase: statistics.median(value["inclusive_ns"] / 1e6 for value in values)
                                               for phase, values in phases.items()},
            }
        comparisons.append({"rows": rows, "measured": measured, "pipeline": pipeline, "profile": enabled,
                            "after_over_before": summaries["after"]["median_ms_per_update"] /
                            summaries["before"]["median_ms_per_update"], "variants": summaries})
    result = {"schema": 2, "pairs": args.pairs, "quick": args.quick, "scroll": args.scroll,
              "measure_limit": args.measure_limit,
              "binaries": {name: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                           for name, path in binaries.items()},
              "scope": "offscreen CPU; pixel hashing outside timer; inclusive phases overlap; no FPS/RSS",
              "images_and_mounted_budgets_equal": True, "positions_and_boxes_equal": True,
              "comparisons": comparisons, "observations": observations}
    (args.out / "evidence.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(comparisons, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
