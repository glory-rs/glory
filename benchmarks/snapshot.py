"""Freeze owned benchmark/library inputs on D: before independent builds."""
from pathlib import Path
import argparse, datetime, hashlib, json, shutil, subprocess, tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', required=True, type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
target = args.out.resolve()
assert target.as_posix().lower().startswith('d:/buildcache/'), target
assert not target.exists(), target
members = tomllib.loads((root/'Cargo.toml').read_text())['workspace']['members']
files = [root/'Cargo.toml', root/'Cargo.lock']
for member in members:
    files.extend(p for p in (root/member).rglob('*') if p.is_file() and 'target' not in p.parts)
files.extend(root/'benchmarks'/name for name in ['data.rs','bench-harness.js','semantic.css'])
for app in ['glory','leptos','dioxus']:
    base=root/'benchmarks'/app
    files.extend(base/name for name in ['Cargo.toml','Cargo.lock','index.html'])
    files.extend(base.glob('src/**/*.rs'))
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
before={p.relative_to(root).as_posix():sha(p) for p in files}
for path in files:
    dest=target/path.relative_to(root)
    dest.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(path,dest)
for name,digest in before.items():
    assert sha(root/name)==digest and sha(target/name)==digest, f'Input changed during snapshot: {name}'
record={'captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'source_root':str(root),'snapshot':str(target),'sha256':before,
        'git_head':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),
        'working_changes':subprocess.check_output(['git','-C',str(root),'status','--short'],text=True)}
(target/'inputs.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'snapshot':str(target),'inputs':len(before)}))
