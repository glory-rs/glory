"""Run the existing DSL suite from a fresh workspace using only registry crates."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--tag',required=True)
options=parser.parse_args()
plan=json.loads((root/'release.json').read_text(encoding='utf-8'))
target=Path(os.environ['CARGO_TARGET_DIR']).resolve()
if os.name=='nt' and not target.as_posix().lower().startswith('d:/buildcache/'):
    raise SystemExit('Registry consumer artifacts must stay under D:/BuildCache')
directory=Path(tempfile.mkdtemp(prefix='registry-consumer-',dir=target.parent))
is_sapid=plan['repository']=='sapid-ui/sapid'
manifest=tomllib.loads((root/'Cargo.toml').read_text(encoding='utf-8'))
version=manifest.get('workspace',{}).get('package',{}).get('version') or manifest['package']['version']
glory_version=manifest['dependencies']['glory']['version'] if is_sapid else '='+version
dependencies='glory = { version = "'+glory_version+'", default-features = false, features = ["dsl", "ssr", "stream", "apace-raster"] }\n'
if is_sapid:
    dependencies+='sapid = "='+version+'"\n'
(directory/'Cargo.toml').write_text('[package]\nname = "registry-consumer"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n'
    '[workspace]\n[features]\ndefault = ["stream"]\nstream = []\n[dependencies]\n'+dependencies,encoding='utf-8')
sources=['tests/dsl.rs','examples/fixtures/dsl_demo.rs'] if is_sapid else ['crates/glory/tests/display_dsl.rs']
hashes={}
for relative in sources:
    source=root/relative
    destination=directory/('tests/display_dsl.rs' if relative.startswith('crates/') else relative)
    destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,destination)
    hashes[relative]=hashlib.sha256(source.read_bytes()).hexdigest()
def cargo(*args,capture=False):
    return subprocess.check_output(['cargo',*args],cwd=directory,text=True,encoding='utf-8') if capture else subprocess.run(['cargo',*args],cwd=directory,check=True)
metadata=json.loads(cargo('metadata','--no-deps','--format-version','1',capture=True))
if Path(metadata['target_directory']).resolve()!=target:
    raise SystemExit('Unexpected registry consumer target')
cargo('test')
# Include the native dependency graph even though the DSL suite uses SSR and the
# headless semantic host: the published adapter must compile from registry crates.
cargo('check','--locked','--all-targets')
lock=tomllib.loads((directory/'Cargo.lock').read_text(encoding='utf-8'))
for package in lock['package']:
    if package['name']=='registry-consumer':
        continue
    if package.get('source')!='registry+https://github.com/rust-lang/crates.io-index':
        raise SystemExit('Consumer acquired a dependency outside crates.io: '+package['name'])
evidence=target/'release'/options.tag/'registry-consumer.json'
evidence.parent.mkdir(parents=True,exist_ok=True)
shutil.copyfile(directory/'Cargo.lock',evidence.with_name('registry-consumer.lock'))
evidence.write_text(json.dumps({'workspace':str(directory),'repository':plan['repository'],'tag':options.tag,
    'test_sources_sha256':hashes,'lock_sha256':hashlib.sha256((directory/'Cargo.lock').read_bytes()).hexdigest(),
    'registry_only':True},indent=2)+'\n',encoding='utf-8')
print('Registry-only DSL/SSR/native consumer verified:',evidence)
