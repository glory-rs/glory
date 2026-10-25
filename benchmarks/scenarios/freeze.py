"""Freeze the active workspace and this standalone application suite on D:."""
import argparse, datetime, hashlib, json, pathlib, shutil, subprocess, tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', required=True, type=pathlib.Path)
out = parser.parse_args().out.resolve()
root = pathlib.Path(__file__).resolve().parents[2]
assert out.as_posix().lower().startswith('d:/buildcache/') and not out.exists()
members = tomllib.loads((root/'Cargo.toml').read_text())['workspace']['members']
files = [root/'Cargo.toml', root/'Cargo.lock']
for base in [*(root/member for member in members), root/'benchmarks/scenarios']:
    files += [p for p in base.rglob('*') if p.is_file() and not {'target','__pycache__'}.intersection(p.parts)]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
before = {p.relative_to(root).as_posix():sha(p) for p in files}
for p in files:
    destination=out/p.relative_to(root)
    destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(p,destination)
for name,digest in before.items():
    assert sha(root/name)==digest and sha(out/name)==digest,name
(out/'inputs.json').write_text(json.dumps({'captured_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'source_root':str(root),'sha256':before,'git_head':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip()},indent=2),encoding='utf-8')
print('Frozen',len(before),'inputs at',out)
