"""Exercise real native file watching, failed build survival, restart and Ctrl+C.

Pass a built glory-cli bootstrap example and the real Glory workspace. All
fixture sources, Cargo state and results stay under the approved build root.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--runner', type=Path, required=True)
parser.add_argument('--glory-source', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
root = args.out.resolve()
if not root.is_relative_to('/Volumes/Data/Caches/glory'):
    parser.error('use the approved Apple build root')
root.mkdir(parents=True, exist_ok=True)
fixture = root/'source'
(fixture/'src').mkdir(parents=True, exist_ok=True)
manifest = fixture/'Cargo.toml'
manifest.write_text(f'''[package]
name = "native-watch-fixture"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
glory = {{ path = {json.dumps(str(args.glory_source.resolve()/'crates/glory'))}, default-features = false }}
[package.metadata.glory.desktop]
bin = "native-watch-fixture"
''')
report = root/'application.json'

def source(generation):
    return '''fn main() {
    let _ = glory::style::Style::default();
    std::fs::write(std::env::var_os("GLORY_NATIVE_WATCH_REPORT").unwrap(),
        format!("{{\\"pid\\":{},\\"generation\\":GEN}}",std::process::id())).unwrap();
    loop { std::thread::sleep(std::time::Duration::from_secs(1)); }
}'''.replace('GEN', str(generation))

entry = fixture/'src/main.rs'
entry.write_text(source(1))
environment = os.environ.copy()
environment.update(CARGO_TARGET_DIR=str(root/'setup-target'), GLORY_NATIVE_WATCH_REPORT=str(report))
subprocess.run(['cargo','generate-lockfile','--manifest-path',str(manifest)],env=environment,check=True,stdout=subprocess.DEVNULL)
logpath = root/'native-watch.log'

def wait_for(predicate, description):
    deadline = time.monotonic()+120
    while time.monotonic() < deadline:
        if predicate(): return
        if process.poll() is not None: raise RuntimeError(f'CLI exited before {description}; inspect {logpath}')
        time.sleep(.1)
    raise RuntimeError(f'timed out waiting for {description}; inspect {logpath}')

def current():
    try: return json.loads(report.read_text())
    except (FileNotFoundError,json.JSONDecodeError): return {}

with logpath.open('w') as log:
    process = subprocess.Popen([str(args.runner.resolve()),'serve','--platform','desktop',
        '--manifest-path',str(manifest),'--build-root',str(root/'build')],env=environment,stdout=log,stderr=log)
    try:
        wait_for(lambda: current().get('generation')==1,'initial application')
        first=current()
        entry.write_text('fn main() { invalid Rust here }')
        wait_for(lambda:'keeping the current application' in logpath.read_text(),'failed build')
        os.kill(first['pid'],0)
        assert current()==first, 'failed build replaced the live application'
        entry.write_text(source(2))
        wait_for(lambda:current().get('generation')==2,'successful restart')
        second=current()
        assert second['pid']!=first['pid']
        process.send_signal(signal.SIGINT)
        process.wait(timeout=20)
        assert process.returncode==0
        try: os.kill(second['pid'],0)
        except ProcessLookupError: pass
        else: raise AssertionError('Ctrl+C leaked the owned application')
        (root/'result.json').write_text(json.dumps(dict(initial=first,restarted=second,
            failed_build_kept_process=True,stop_reaped_process=True),indent=2)+'\n')
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
            process.wait(timeout=20)
print(root/'result.json')
