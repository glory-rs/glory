"""Build frozen old/new vector Each APIs, then compare identical headless UI work."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--build-only', action='store_true')
parser.add_argument('--run-only', action='store_true')
args = parser.parse_args()
out = args.out.resolve()
assert out.as_posix().lower().startswith('d:/buildcache/'), out
out.mkdir(parents=True, exist_ok=True)
(out / 'tmp').mkdir(exist_ok=True)
env = dict(os.environ, CARGO_HOME='D:/BuildCache/glory-semantic-ui/cargo',
           CARGO_TARGET_DIR=str(out / 'target'), TEMP=str(out / 'tmp'), TMP=str(out / 'tmp'),
           CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2')
source = r'''
use glory_ui::{App, each, text};
use std::time::Instant;
fn main() {
    for sample in 0..15 {
        let app=App::memory();
        let values=app.scope().cage((0..1000usize).map(|id|(id,format!("row {id}"))).collect::<Vec<_>>());
        let start=Instant::now();
        app.mount(EACH).unwrap();app.flush().unwrap();
        let create=start.elapsed().as_nanos();
        let live=app.runtime_stats();
        let root=app.tree().roots()[0];
        let first=app.tree().node(root).unwrap().children[0];
        let start=Instant::now();
        values.revise(|rows| for row in rows.iter_mut().step_by(10) { row.1.push_str(" !!!"); });
        app.flush().unwrap();let update=start.elapsed().as_nanos();
        let start=Instant::now();values.revise(|rows|rows.swap(0,998));app.flush().unwrap();
        let swap=start.elapsed().as_nanos();
        assert_eq!(app.tree().node(root).unwrap().children[998],first);
        assert_eq!((app.runtime_stats().nodes,app.runtime_stats().owners),(live.nodes,live.owners));
        let start=Instant::now();values.revise(Vec::clear);app.flush().unwrap();
        let clear=start.elapsed().as_nanos();
        assert!(app.tree().node(root).unwrap().children.is_empty());
        app.clear().unwrap();assert_eq!(app.stats().nodes,0);
        if sample>=3 {println!("{{\"sample\":{},\"create\":{},\"update\":{},\"swap\":{},\"clear\":{},\"nodes\":{},\"owners\":{}}}",sample-3,create,update,swap,clear,live.nodes,live.owners);}
    }
}
'''
executables = {}
for name, snapshot in [('before', args.before), ('after', args.after)]:
    snapshot = snapshot.resolve()
    assert (snapshot / 'inputs.json').is_file(), snapshot
    project = out / name
    (project / 'src').mkdir(parents=True, exist_ok=True)
    expression = ('each(values, |row| row.0, |value, _| text().content(value.map(|row|row.1.clone())))'
                  if name == 'before' else
                  'each(values).key(|row| row.0).children(|row| text().content(row.value.map(|row|row.1.clone())))')
    (project / 'src/main.rs').write_text(source.replace('EACH', expression), encoding='utf-8')
    manifest = project / 'Cargo.toml'
    manifest.write_text('[package]\nname="each-vector-' + name + '"\nversion="0.0.0"\nedition="2024"\n'
                        '[workspace]\n[dependencies]\nglory-ui={path=' + json.dumps((snapshot / 'crates/ui').as_posix()) + '}\n', encoding='utf-8')
    metadata = subprocess.run(['cargo', 'metadata', '--manifest-path', str(manifest), '--no-deps', '--format-version', '1'],
                              env=env, capture_output=True, text=True, check=True)
    assert Path(json.loads(metadata.stdout)['target_directory']).resolve() == (out / 'target').resolve()
    if not args.run_only:
        subprocess.run(['cargo', 'build', '--offline', '--release', '--manifest-path', str(manifest)], env=env, check=True)
    executables[name] = out / 'target/release' / ('each-vector-' + name + '.exe')
if args.build_only:
    raise SystemExit(0)
for process in ['cargo.exe', 'rustc.exe']:
    active = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq ' + process, '/NH'], capture_output=True, text=True).stdout.lower()
    assert process not in active, 'finish compiler work before measuring'
runs = []
for round_index in range(4):
    for name in (['before', 'after'] if round_index % 2 == 0 else ['after', 'before']):
        result = subprocess.run([str(executables[name])], env=env, capture_output=True, text=True, check=True)
        samples = [json.loads(line) for line in result.stdout.splitlines()]
        assert len(samples) == 12
        runs.append(dict(round=round_index, name=name, samples=samples))
summary = {}
for key in ['create', 'update', 'swap', 'clear']:
    medians = {name: statistics.median(sample[key] for run in runs if run['name'] == name for sample in run['samples']) / 1e6
               for name in executables}
    summary[key] = dict(ms=medians, change_percent=100 * (medians['after'] / medians['before'] - 1))
assert len({(s['nodes'], s['owners']) for run in runs for s in run['samples']}) == 1
evidence = dict(before=str(args.before.resolve()), after=str(args.after.resolve()), workload='headless vector Each 1000 semantic text rows',
                warmup=3, samples=12, rounds=4, runs=runs, summary=summary,
                harness_sha256={name: hashlib.sha256((out / name / 'src/main.rs').read_bytes()).hexdigest() for name in executables})
(out / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary))
