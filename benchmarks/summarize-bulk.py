"""Verify and summarize the paired full-DOM samples (no third-party packages)."""
import argparse
import hashlib
import json
import statistics
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
assert args.out.resolve().as_posix().lower().startswith('d:/buildcache/')
data = json.loads(args.evidence.read_text(encoding='utf-8'))
rounds = data['rounds']
assert [(row['round'], row['variant']) for row in rounds] == [
    (0, 'before'), (0, 'release'), (1, 'release'), (1, 'before')]
canonical = {}
for run in rounds:
    assert not run['errors']
    assert run['warmup'] == 3 and run['samples'] == 12
    assert len(run['results']) == 9
    for operation in run['results']:
        assert len(operation['samples']) == 12
        for index, sample in enumerate(operation['samples']):
            key = operation['key'], index
            value = sample['count'], sample['sha256']
            assert canonical.setdefault(key, value) == value

def samples(variant, key):
    return [sample for run in rounds if run['variant'] == variant
            for operation in run['results'] if operation['key'] == key
            for sample in operation['samples']]

rows = []
for operation in rounds[0]['results']:
    key = operation['key']
    row = {'key': key}
    for variant in ['before', 'release']:
        values = samples(variant, key)
        row[variant] = {
            'samples': len(values),
            'wall_ms': statistics.median(value['elapsed_ms'] for value in values),
            'script_ms': statistics.median(value['metrics']['ScriptDuration'] for value in values),
            'layout_style_ms': statistics.median(
                value['metrics']['LayoutDuration'] + value['metrics']['RecalcStyleDuration']
                for value in values),
        }
    row['wall_reduction_percent'] = 100 * (1 - row['release']['wall_ms'] / row['before']['wall_ms'])
    row['script_reduction_percent'] = 100 * (1 - row['release']['script_ms'] / row['before']['script_ms'])
    rows.append(row)
    print(f"| {key} | {row['before']['wall_ms']:.2f} | {row['release']['wall_ms']:.2f} "
          f"| {row['wall_reduction_percent']:.1f}% | {row['before']['script_ms']:.2f} "
          f"| {row['release']['script_ms']:.2f} | {row['script_reduction_percent']:.1f}% |")

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

root = args.evidence.resolve().parent.parent
sources = {}
builds = {}
for variant in ['before', 'release']:
    source = root / variant / 'source'
    record = json.loads((source / 'inputs.json').read_text(encoding='utf-8'))
    for name, digest in record['sha256'].items():
        assert sha(source / name) == digest, name
    sources[variant] = record
    artifacts = [json.loads(line) for line in (root / variant / 'glory-build.jsonl').read_text(encoding='utf-8-sig').splitlines()]
    application = [row for row in artifacts if row.get('reason') == 'compiler-artifact'
                   and row['target']['name'] == 'bench-glory' and 'bin' in row['target']['kind']][-1]
    assert not application['fresh'], 'Each measured application must be freshly built'
    assert Path(application['manifest_path']).resolve() == (source / 'benchmarks/glory/Cargo.toml').resolve()
    metadata = json.loads((root / variant / 'glory-metadata-final.json').read_text(encoding='utf-8-sig'))
    assert Path(metadata['target_directory']).resolve() == (root / variant / 'glory/target').resolve()
    wasm = root / variant / 'site/glory/app_bg.wasm'
    builds[variant] = {'manifest': application['manifest_path'], 'fresh': application['fresh'],
                       'target': metadata['target_directory'], 'wasm_bytes': wasm.stat().st_size,
                       'wasm_sha256': sha(wasm)}
    for name in ['bench-harness.js', 'semantic.css']:
        assert sha(root / variant / 'site/glory' / name) == record['sha256']['benchmarks/' + name]
before, after = (sources[variant]['sha256'] for variant in ['before', 'release'])
workload = ['benchmarks/data.rs', 'benchmarks/bench-harness.js', 'benchmarks/semantic.css',
            'benchmarks/glory/Cargo.toml', 'benchmarks/glory/Cargo.lock',
            'benchmarks/glory/index.html', 'benchmarks/glory/src/main.rs']
assert all(before[name] == after[name] for name in workload)
assert data['driver_sha256'] == sha(args.evidence.parent / 'driver-source/bulk-compare.cjs')
assert data['harness_sha256'] == after['benchmarks/bench-harness.js']
assert data['acceptance'][0]['pictures'] == data['acceptance'][1]['pictures']
summary = {'contract': data['contract'], 'browser': data['browser'], 'samples': 432,
           'warmups': 108, 'rows': rows, 'builds': builds,
           'workload_sha256': {name: after[name] for name in workload},
           'changed_existing_inputs': [name for name in before if before[name] != after[name]],
           'new_inputs': [name for name in after if name not in before],
           'evidence_sha256': sha(args.evidence)}
args.out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
