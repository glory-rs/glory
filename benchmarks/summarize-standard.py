"""Summarize a complete frozen standard-suite run without discarding samples."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("run", type=Path)
parser.add_argument("--out", type=Path, required=True)
parser.add_argument("--markdown", type=Path)
args = parser.parse_args()
metadata = json.loads((args.run / "inputs.json").read_text(encoding="utf-8"))
assert metadata.get("completed") and metadata["result_files"] == len(metadata["frameworks"]) * 15
results = [json.loads(file.read_text(encoding="utf-8"))
           for file in sorted((args.run / "results").glob("*.json"))]
assert len(results) == len(metadata["frameworks"]) * 15
names = metadata["frameworks"]
def name_of(result):
    return next(name for name in names if result["framework"] == name + "-keyed"
                or result["framework"].startswith(name + "-v"))
by_case = {}
for result in results:
    by_case.setdefault(result["benchmark"], {})[name_of(result)] = result
assert all(set(items) == set(names) for items in by_case.values())
summary = {"schema": 1, "evidence": str(args.run.resolve()), "metadata": metadata,
           "statistic": "Median; upstream values, means and all slow samples retained",
           "results": results}
args.out.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
tables = []
for kind, metric, title in [("cpu", "total", "CPU total 中位数（ms）"),
                             ("cpu", "script", "CPU Script 中位数（ms）"),
                             ("memory", "DEFAULT", "页面归属内存（MiB；每项仅 1 样本）"),
                             ("size", "DEFAULT", "体积（KiB）与 first paint（ms；每项仅 1 样本）")]:
    tables += [title, "", "| 项目 | " + " | ".join(names) + " |",
               "|---|" + "---:|" * len(names)]
    for case, items in sorted(by_case.items()):
        if next(iter(items.values()))["type"] != kind:
            continue
        tables.append("| " + case + " | " + " | ".join(
            f'{items[name]["values"][metric]["median"]:.2f}' for name in names) + " |")
    tables.append("")
if args.markdown:
    args.markdown.write_text("\n".join(tables), encoding="utf-8")
print(f"Preserved {len(results)} results and {len(by_case)} complete cases: {args.out}")
