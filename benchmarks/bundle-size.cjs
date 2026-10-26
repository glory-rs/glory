// Measure existing semantic-rows-v1 releases; never changes their Cargo profiles.
'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),crypto=require('node:crypto'),zlib=require('node:zlib');
const {javascriptMetrics}=require('./wasm-size/javascript.cjs');
const args=process.argv.slice(2),option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const root=path.resolve(option('--build-root','D:/BuildCache/glory-benchmark-wasm-20261026'));
const apps=option('--apps','glory,dioxus,leptos').split(',');
if(!apps.length||new Set(apps).size!==apps.length||apps.some(app=>!['glory','dioxus','leptos'].includes(app)))throw Error('Unknown/duplicate app');
const out=path.join(root,'bundle-size');
if(!root.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Build root must be D:/BuildCache');
if(fs.existsSync(path.join(out,'report.json')))throw Error('Use a new build root to preserve previous results');
fs.mkdirSync(out,{recursive:true});
const binaryen=path.resolve(option('--binaryen-dir','D:/BuildCache/sapid/wasm-size/tools/node_modules/binaryen'));
if(JSON.parse(fs.readFileSync(path.join(binaryen,'package.json'))).version!=='132.0.0')throw Error('Use Binaryen 132.0.0');
const optimizer=path.join(binaryen,'bin/wasm-opt');
const env={...process.env,TEMP:path.join(root,'tmp'),TMP:path.join(root,'tmp')};
function run(exe,argv){const r=cp.spawnSync(exe,argv,{env,encoding:'utf8',windowsHide:true,maxBuffer:32*1024*1024});if(r.status!==0)throw Error(`${exe}: ${r.stderr||r.error}`);return r.stdout.trim();}
function hash(bytes){return crypto.createHash('sha256').update(bytes).digest('hex');}
function measure(file){
  const b=fs.readFileSync(file),br=zlib.brotliCompressSync(b,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11,[zlib.constants.BROTLI_PARAM_MODE]:zlib.constants.BROTLI_MODE_GENERIC}}),gz=zlib.gzipSync(b,{level:9,mtime:0});
  fs.writeFileSync(file+'.br',br);fs.writeFileSync(file+'.gz',gz);
  return {bytes:b.length,brotli:br.length,gzip:gz.length,sha256:hash(b)};
}
function sections(file){
  const b=fs.readFileSync(file),r=[];let p=8;
  function leb(){let n=0,s=0,x;do{x=b[p++];n+=(x&127)*2**s;s+=7;}while(x&128);return n;}
  while(p<b.length){const start=p,id=b[p++],n=leb(),payload=p,end=p+n;let name;
    if(id===0){const len=leb();name=b.toString('utf8',p,p+len);}
    r.push({id,...(name?{name}:{}),payloadBytes:n,encodedBytes:end-start,sha256:hash(b.subarray(payload,end))});p=end;
  }return r;
}
const metadata=JSON.parse(fs.readFileSync(path.join(root,apps[0]+'-metadata-final.json'),'utf8').replace(/^\uFEFF/,''));
const source=path.resolve(path.dirname(metadata.workspace_root),'..');
const inputManifest=path.resolve(option('--source-manifest',path.join(source,'inputs.json')));
const inputs=JSON.parse(fs.readFileSync(inputManifest));
const report={schema:1,recorded:new Date().toISOString(),contract:'semantic-rows-v1',source:{snapshot:path.dirname(inputManifest),compiledAt:source,gitHead:inputs.git_head,workingChanges:inputs.working_changes,inputDigest:hash(JSON.stringify(inputs.sha256))},
  versions:{rustc:run('rustc',['-Vv']),cargo:run('cargo',['-V']),node:process.version,binaryen:run(process.execPath,[optimizer,'--version']),wasmBindgen:'0.2.129'},
  profile:{optLevel:3,lto:true,codegenUnits:1},optimizer:'-Oz --enable-bulk-memory --enable-reference-types',compression:{brotliQuality:11,gzipLevel:9},
  scope:'Actual rows benchmark; JS includes entry and transitively imported relative JS snippets, separately compressed; common harness, CSS and HTML excluded.',cases:[]};
for(const app of apps){
  console.log(`Measuring ${app}`);
  const messages=fs.readFileSync(path.join(root,app+'-build.jsonl'),'utf8').replace(/^\uFEFF/,'').split(/\r?\n/).filter(Boolean).map(line=>JSON.parse(line));
  const artifact=messages.findLast(m=>m.reason==='compiler-artifact'&&m.target.name==='bench-'+app&&m.target.kind.includes('bin'));
  if(!artifact||artifact.profile.opt_level!=='3'||artifact.profile.debuginfo!==0)throw Error('Unexpected release artifact/profile');
  const dir=path.join(out,app);fs.mkdirSync(dir,{recursive:true});
  const site=path.join(root,'site',app),wasm=path.join(site,'app_bg.wasm');
  fs.copyFileSync(artifact.filenames.find(f=>f.endsWith('.wasm')),path.join(dir,'rustc.wasm'));
  fs.copyFileSync(wasm,path.join(dir,'bindgen.wasm'));
  run(process.execPath,[optimizer,wasm,'--strip-debug','--enable-bulk-memory','--enable-reference-types','-o',path.join(dir,'stripped.wasm')]);
  run(process.execPath,[optimizer,wasm,'-Oz','--enable-bulk-memory','--enable-reference-types','-o',path.join(dir,'optimized.wasm')]);
  const js=javascriptMetrics(site,'app.js');
  const packageId=messages.find(m=>m.reason==='compiler-artifact'&&m.target.name===app)?.package_id;
  if(!packageId)throw Error('Missing framework artifact identity');
  const version=packageId.match(/(?:#|@)(\d+\.\d+\.\d+(?:[-+]\S*)?)$/)?.[1]||packageId;
  const item={name:app,version,packageId,lockSha256:hash(fs.readFileSync(path.join(source,'benchmarks',app,'Cargo.lock'))),
    rustc:measure(path.join(dir,'rustc.wasm')),bindgen:measure(path.join(dir,'bindgen.wasm')),stripped:measure(path.join(dir,'stripped.wasm')),optimized:measure(path.join(dir,'optimized.wasm')),js,
    sections:{bindgen:sections(wasm),optimized:sections(path.join(dir,'optimized.wasm'))},profile:artifact.profile};
  report.cases.push(item);
  for(const stage of ['bindgen','optimized']){
    const dest=path.join(out,'site',stage,app);fs.cpSync(site,dest,{recursive:true});
    const wasmSource=path.join(dir,stage+'.wasm');fs.copyFileSync(wasmSource,path.join(dest,'app_bg.wasm'));
    for(const ext of ['.br','.gz'])fs.copyFileSync(wasmSource+ext,path.join(dest,'app_bg.wasm'+ext));
  }
  console.log(`  bindgen ${item.bindgen.bytes}; strip ${item.stripped.bytes}; -Oz ${item.optimized.bytes}; Brotli ${item.optimized.brotli}; JS ${js.bytes}`);
}
fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2)+'\n');
const rows=['| Framework | bindgen WASM | names stripped | -Oz WASM | -Oz Brotli | -Oz gzip | JS | WASM+JS Brotli |','|---|---:|---:|---:|---:|---:|---:|---:|'];
for(const r of report.cases)rows.push(`| ${r.name} | ${r.bindgen.bytes} | ${r.stripped.bytes} | ${r.optimized.bytes} | ${r.optimized.brotli} | ${r.optimized.gzip} | ${r.js.bytes} | ${r.optimized.brotli+r.js.brotli} |`);
fs.writeFileSync(path.join(out,'summary.md'),rows.join('\n')+'\n');
console.log(path.join(out,'report.json'));
