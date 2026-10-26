// Node 22+, wasm-bindgen 0.2.129, Binaryen npm 132.0.0. No npm deps in the repos.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const crypto = require('node:crypto');
const zlib = require('node:zlib');
const {javascriptMetrics}=require('./javascript.cjs');
const {compare,enforce}=require('./compare.cjs');
const fixture = __dirname;
const glory = path.resolve(fixture, '../..');
const sapid = path.resolve(glory, '../sapid');
const options = process.argv.slice(2);
function arg(name, fallback) { const i = options.indexOf(name); return i < 0 ? fallback : options[i + 1]; }
const root = path.resolve(arg('--build-root', 'D:/BuildCache/sapid/wasm-size'));
if (!root.replaceAll('\\', '/').toLowerCase().startsWith('d:/buildcache/')) throw Error('Build root must be on D:/BuildCache');
const label = arg('--label', 'baseline');
if (!/^[a-z0-9-]+$/.test(label)) throw Error('Invalid run label');
const out = path.join(root, 'runs', label);
if(fs.existsSync(path.join(out,'report.json'))||fs.existsSync(path.join(out,'source')))throw Error('Use a new run label to preserve previous evidence');
const target = path.join(root, 'target');
const source = path.join(root, 'source');
const manifest = path.join(source, 'glory/benchmarks/wasm-size/Cargo.toml');
const binaryen = path.resolve(arg('--binaryen-dir', path.join(root, 'tools/node_modules/binaryen')));
const wasmOpt = path.join(binaryen, 'bin/wasm-opt');
const bindgen = arg('--bindgen', process.env.WASM_BINDGEN || 'wasm-bindgen');
const env = { ...process.env, CARGO_HOME: 'D:/BuildCache/glory-semantic-ui/cargo', CARGO_TARGET_DIR: target,
  TEMP: path.join(root, 'tmp'), TMP: path.join(root, 'tmp'), TMPDIR: path.join(root, 'tmp'),
  PYTHONPYCACHEPREFIX: path.join(root, 'pycache'), npm_config_cache: path.join(root, 'npm-cache') };
for (const key of Object.keys(env)) {
  if (/^CARGO_PROFILE_RELEASE_|^CARGO_TARGET_.*_RUSTFLAGS$/.test(key) || ['RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP'].includes(key)) delete env[key];
}
for (const dir of [out, env.TEMP, target]) fs.mkdirSync(dir, { recursive: true });
const lockFile=path.join(root,'runner.lock');
const lockFd=fs.openSync(lockFile,'wx');
fs.writeFileSync(lockFd,JSON.stringify({pid:process.pid,label}));fs.closeSync(lockFd);
process.on('exit',()=>{try{if(JSON.parse(fs.readFileSync(lockFile)).pid===process.pid)fs.unlinkSync(lockFile);}catch{}});
process.on('SIGINT',()=>process.exit(130));process.on('SIGTERM',()=>process.exit(143));
function run(exe, args, extra = {}) {
  const result = cp.spawnSync(exe, args, { cwd: glory, env, encoding: 'utf8', windowsHide: true, maxBuffer: 32 * 1024 * 1024, ...extra });
  if (result.status !== 0) {
    fs.writeFileSync(path.join(out,'failure.log'),`${result.stdout}\n${result.stderr}`);
    const diagnostics = String(result.stdout).split('\n').flatMap(line=>{try {const message=JSON.parse(line);return message.reason==='compiler-message'?[message.message.rendered]:[];}catch{return [];}}).join('');
    throw Error(`${exe} ${args.join(' ')}\n${result.error || ''}\n${diagnostics || String(result.stdout).slice(-3000)}\n${result.stderr}`);
  }
  return result.stdout;
}
function hash(data) { return crypto.createHash('sha256').update(data).digest('hex'); }
const excluded = new Set(['.git', 'target', 'node_modules', '__pycache__']);
function copy(from, to, entries) {
  fs.mkdirSync(to, { recursive: true });
  for (const item of fs.readdirSync(from, { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name, 'en'))) {
    if (excluded.has(item.name)) continue;
    const a = path.join(from, item.name), b = path.join(to, item.name);
    if (item.isDirectory()) copy(a, b, entries);
    else if (item.isFile()) {
      const bytes = fs.readFileSync(a);
      fs.writeFileSync(b, bytes);
      entries.push([a.replaceAll('\\','/'), hash(bytes)]);
    } else throw Error(`Unsupported source entry: ${a}`);
  }
}
function clearStaging() {
  // Generated staging only: reject junctions and verify the absolute D: target
  // before a recursive replacement. Per-run archives and outputs are separate.
  if(path.dirname(source)!==root||path.basename(source)!=='source'||!root.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/')) throw Error('Invalid staging target');
  if(fs.existsSync(source)&&fs.realpathSync(source).toLowerCase()!==source.toLowerCase()) throw Error('Staging must not be a junction');
  fs.rmSync(source,{recursive:true,force:true});
}
function snapshot() {
  clearStaging();
  const entries = [];
  for (const [repo, name, dirs] of [[glory, 'glory', ['crates', 'benchmarks/wasm-size']], [sapid, 'sapid', ['src']]]) {
    const dest = path.join(source, name);
    fs.mkdirSync(dest, { recursive: true });
    for (const file of ['Cargo.toml', 'Cargo.lock', 'README.md', 'AGENTS.md']) {
      if (fs.existsSync(path.join(repo, file))) {
        const bytes = fs.readFileSync(path.join(repo, file));
        fs.writeFileSync(path.join(dest, file), bytes);
        entries.push([path.join(repo,file).replaceAll('\\','/'), hash(bytes)]);
      }
    }
    for (const dir of dirs) copy(path.join(repo, dir), path.join(dest, dir), entries);
  }
  // A single frozen source path is reused between runs so embedded Rust paths
  // cannot masquerade as implementation size differences.
  return { digest: hash(JSON.stringify(entries)), files: entries, roots: {glory,sapid},
    gloryRevision: run('git', ['-C', glory, 'rev-parse', 'HEAD']).trim(),
    sapidRevision: run('git', ['-C', sapid, 'rev-parse', 'HEAD']).trim(),
    gloryStatus: run('git', ['-C', glory, 'status', '--short']), sapidStatus: run('git', ['-C', sapid, 'status', '--short']) };
}
function replaySnapshot(from) {
  const archive=path.resolve(from);
  if(archive===source||archive.startsWith(source+path.sep)||source.startsWith(archive+path.sep)) throw Error('Replay archive must be independent of the staging directory');
  const info=JSON.parse(fs.readFileSync(path.join(archive,'../sources.json')));
  if(info.digest!==hash(JSON.stringify(info.files))) throw Error('Archived source manifest digest mismatch');
  const roots=info.roots||{glory,sapid};
  const restored=[];
  for(const [file,digest] of info.files) {
    const rootName=Object.keys(roots).find(name=>file.startsWith(roots[name].replaceAll('\\','/')+'/'));
    if(!rootName) throw Error(`Unknown archived source root: ${file}`);
    const relative=file.slice(roots[rootName].replaceAll('\\','/').length+1);
    if(hash(fs.readFileSync(path.join(archive,rootName,relative)))!==digest) throw Error(`Archived source changed: ${file}`);
    restored.push(path.join(source,rootName,relative));
  }
  clearStaging();
  fs.cpSync(archive,source,{recursive:true});
  // Windows file copies can retain an old LastWriteTime. Cargo's freshness
  // checks must see restored inputs as newer than the artifacts they replace.
  const restoredAt=new Date();
  for(const file of restored)fs.utimesSync(file,restoredAt,restoredAt);
  return {...info,replayedFrom:archive};
}
const snapshotInfo = arg('--source-from','')?replaySnapshot(arg('--source-from','')):snapshot();
// Keep dirty local sources, not just Git SHAs, so an earlier result remains
// reproducible after later optimizations overwrite the common compilation path.
fs.cpSync(source, path.join(out, 'source'), { recursive: true });
if (!fs.existsSync(wasmOpt) || JSON.parse(fs.readFileSync(path.join(binaryen, 'package.json'))).version !== '132.0.0') {
  throw Error(`Install pinned optimizer: npm install --prefix "${path.join(root,'tools')}" --save-exact binaryen@132.0.0 (set npm_config_cache on D:)`);
}
const versions = { rustc: run('rustc', ['-Vv']).trim(), cargo: run('cargo',['-V']).trim(),
  bindgen: run(bindgen,['--version']).trim(), binaryen: run(process.execPath,[wasmOpt,'--version']).trim(), node: process.version };
if (versions.bindgen !== 'wasm-bindgen 0.2.129') throw Error('wasm-bindgen library/CLI version mismatch');
const metadata = JSON.parse(run('cargo', ['metadata', '--manifest-path', manifest, '--no-deps', '--format-version', '1']));
if (path.resolve(metadata.target_directory).toLowerCase() !== target.toLowerCase()) throw Error('Cargo output is outside the verified D: target');
const allCases = JSON.parse(fs.readFileSync(path.join(path.dirname(manifest), 'cases.json')));
const only = arg('--only', '').split(',').filter(Boolean);
const cases = only.length ? allCases.filter(item => only.includes(item.name)) : allCases;
if (!cases.length || only.some(name => !allCases.some(item => item.name === name))) throw Error('Unknown or empty case selection');
fs.writeFileSync(path.join(out, 'sources.json'), JSON.stringify(snapshotInfo, null, 2));
const report = { schema: 1, label, versions, sourceDigest: snapshotInfo.digest, verifiedTarget: metadata.target_directory,
  profile: { optLevel: 'z', lto: true, codegenUnits: 1, strip: 'debuginfo' }, optimizer: '-Oz --enable-bulk-memory --enable-reference-types',
  compression: { brotliQuality: 11, gzipLevel: 9 }, jsScope:'wasm-bindgen entry and transitively imported JS snippets; per-resource compression; excludes test loader', cases: [] };
const previousFile=arg('--compare','');
const previous=previousFile?JSON.parse(fs.readFileSync(previousFile)):null;
if(previous && JSON.stringify(previous.versions)!==JSON.stringify(versions)) throw Error('Size comparisons require the same recorded toolchain');
if(previous && path.resolve(previous.verifiedTarget).toLowerCase()!==target.toLowerCase()) throw Error('Size comparisons require the same D: build/source paths');
if(previous && (JSON.stringify(previous.profile)!==JSON.stringify(report.profile)||JSON.stringify(previous.compression)!==JSON.stringify(report.compression)||previous.optimizer!==report.optimizer)) throw Error('Size comparisons require identical release/optimizer/compression settings');
if (!fs.existsSync(path.join(path.dirname(manifest), 'Cargo.lock'))) run('cargo', ['generate-lockfile', '--manifest-path', manifest]);
fs.copyFileSync(path.join(path.dirname(manifest), 'Cargo.lock'), path.join(out,'Cargo.lock'));
report.lockSha256=hash(fs.readFileSync(path.join(out,'Cargo.lock')));
const fragments = JSON.parse(run('cargo', ['run','--locked','--quiet','--manifest-path',manifest,'--bin','snapshot','--no-default-features','--features','gen-snapshot']));
function metrics(file) {
  const bytes = fs.readFileSync(file);
  const br = zlib.brotliCompressSync(bytes, { params: { [zlib.constants.BROTLI_PARAM_QUALITY]: 11, [zlib.constants.BROTLI_PARAM_MODE]: zlib.constants.BROTLI_MODE_GENERIC } });
  const gz = zlib.gzipSync(bytes, { level: 9, mtime: 0 });
  fs.writeFileSync(file+'.br',br); fs.writeFileSync(file+'.gz',gz);
  return { bytes: bytes.length, brotli: br.length, gzip: gz.length, sha256: hash(bytes) };
}
function sectionSizes(file) {
  const bytes = fs.readFileSync(file); let offset = 8; const result = {};
  function leb() { let value=0, shift=0, part; do {part=bytes[offset++];value+=(part&127)*2**shift;shift+=7;} while(part&128);return value; }
  while(offset < bytes.length) { const id=bytes[offset++], size=leb(); result[id]=(result[id]||0)+size; offset+=size; }
  return result;
}
for (const item of cases) {
  const dir = path.join(out, item.name); fs.mkdirSync(dir, {recursive:true});
  const features = item.features.join(',');
  const buildArgs=['build','--locked','--manifest-path',manifest,'--lib','--release','--target','wasm32-unknown-unknown',
    '--config','profile.release.opt-level="z"','--config','profile.release.lto=true','--config','profile.release.codegen-units=1',
    '--config','profile.release.debug=false','--config','profile.release.strip="debuginfo"','--config','target.wasm32-unknown-unknown.rustflags=[]',
    '--no-default-features',...(features?['--features',features]:[]),'--message-format=json'];
  process.stdout.write(`Building ${item.name}\n`);
  const before=Date.now();
  const build=run('cargo',buildArgs);
  fs.writeFileSync(path.join(dir,'build.jsonl'),build);
  const artifact=build.split('\n').filter(Boolean).map(line=>JSON.parse(line)).find(message=>message.reason==='compiler-artifact'&&message.target.name==='glory_wasm_size'&&message.filenames.some(f=>f.endsWith('.wasm')));
  if(!artifact) throw Error('Missing final WASM artifact');
  if(artifact.profile.opt_level!=='z'||artifact.profile.debuginfo!==0) throw Error('Unexpected effective WASM profile');
  const raw=artifact.filenames.find(f=>f.endsWith('.wasm'));
  fs.copyFileSync(raw,path.join(dir,'rustc.wasm'));
  run(bindgen,[raw,'--target','web','--out-name','fixture','--out-dir',dir]);
  const wasm=path.join(dir,'fixture_bg.wasm');
  fs.copyFileSync(wasm,path.join(dir,'bindgen.wasm'));
  const bindgenMetrics=metrics(path.join(dir,'bindgen.wasm'));
  run(process.execPath,[wasmOpt,wasm,'-o',path.join(dir,'optimized.wasm'),'-Oz','--enable-bulk-memory','--enable-reference-types']);
  const optimizedMetrics=metrics(path.join(dir,'optimized.wasm'));
  fs.copyFileSync(path.join(dir,'optimized.wasm'),wasm);
  fs.copyFileSync(path.join(dir,'optimized.wasm.br'),wasm+'.br');
  fs.copyFileSync(path.join(dir,'optimized.wasm.gz'),wasm+'.gz');
  const jsMetrics=javascriptMetrics(dir);
  const html = `<!doctype html><html><head><meta charset="utf-8"><title>${item.name}</title></head><body><div id="app">${item.features.some(f=>f==='use-hydrate'||f==='use-stream')?fragments.html:''}</div>${item.features.includes('use-stream')?fragments.data:''}<script type="module" src="./loader.js"></script></body></html>`;
  fs.writeFileSync(path.join(dir,'index.html'),html);
  fs.writeFileSync(path.join(dir,'loader.js'),`import init,{mount,unmount} from './fixture.js';\ntry { await init(); const before=[...document.querySelectorAll('#app [data-gly-key]')]; mount(); window.fixture={unmount,before}; document.body.dataset.ready='true'; } catch(error) { document.body.dataset.error=String(error); console.error(error); }\n`);
  fs.writeFileSync(path.join(dir,'features.txt'),run('cargo',['tree','--locked','--manifest-path',manifest,'--target','wasm32-unknown-unknown','--no-default-features',...(features?['--features',features]:[]),'-e','features']));
  report.cases.push({...item, bindgen:bindgenMetrics, optimized:optimizedMetrics, js:jsMetrics, sections:sectionSizes(wasm), buildMs:Date.now()-before, rustcFresh:artifact.fresh});
  fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2));
  process.stdout.write(`  WASM ${optimizedMetrics.bytes}, br ${optimizedMetrics.brotli}; bindgen ${bindgenMetrics.bytes}\n`);
}
const lookup=new Map(report.cases.map(item=>[item.name,item]));
const rows=['| Case | WASM | Brotli | JS | WASM Δ | Brotli Δ |','|---|---:|---:|---:|---:|---:|'];
for(const item of report.cases) { const ref=lookup.get(item.reference); rows.push(`| ${item.name} | ${item.optimized.bytes} | ${item.optimized.brotli} | ${item.js.bytes} | ${ref?item.optimized.bytes-ref.optimized.bytes:'—'} | ${ref?item.optimized.brotli-ref.optimized.brotli:'—'} |`); }
fs.writeFileSync(path.join(out,'summary.md'),rows.join('\n')+'\n');
if(previous) {
  const {comparison,failures}=compare(report,previous);
  fs.writeFileSync(path.join(out,'comparison.json'),JSON.stringify(comparison,null,2));
  if(options.includes('--check')) enforce(failures);
}
process.stdout.write(`Report: ${path.join(out,'report.json')}\n`);
