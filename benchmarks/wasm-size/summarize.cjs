'use strict';
const fs=require('node:fs'),path=require('node:path');
const crypto=require('node:crypto'),zlib=require('node:zlib');
const {javascriptMetrics}=require('./javascript.cjs');
const root=path.resolve(process.argv[2]);
if(!root.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Artifacts must stay on D:');
const file=path.join(root,'report.json'),report=JSON.parse(fs.readFileSync(file));
function hash(bytes){return crypto.createHash('sha256').update(bytes).digest('hex');}
for(const item of report.cases) {
  const dir=path.join(root,item.name);
  item.js=javascriptMetrics(dir);
  const bindgen=path.join(dir,'bindgen.wasm'),main=path.join(dir,'fixture_bg.wasm'),optimized=path.join(dir,'optimized.wasm');
  if(hash(fs.readFileSync(bindgen))!==item.bindgen.sha256||hash(fs.readFileSync(optimized))!==item.optimized.sha256)throw Error(`WASM artifact changed: ${item.name}`);
  for(const [extension,decode,encode] of [
    ['.br',zlib.brotliDecompressSync,bytes=>zlib.brotliCompressSync(bytes,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11,[zlib.constants.BROTLI_PARAM_MODE]:zlib.constants.BROTLI_MODE_GENERIC}})],
    ['.gz',zlib.gunzipSync,bytes=>zlib.gzipSync(bytes,{level:9,mtime:0})],
  ]) {
    if(!fs.existsSync(bindgen+extension)) {
      // Migrate older reports whose primary filename still held the bindgen
      // compressed sidecar. Subsequent calls use the explicit stage names.
      const legacy=fs.existsSync(main+extension)?fs.readFileSync(main+extension):null;
      fs.writeFileSync(bindgen+extension,legacy&&hash(decode(legacy))===item.bindgen.sha256?legacy:encode(fs.readFileSync(bindgen)));
    }
    if(hash(decode(fs.readFileSync(optimized+extension)))!==item.optimized.sha256)throw Error('Compressed optimizer artifact mismatch');
    fs.copyFileSync(optimized+extension,main+extension);
  }
  fs.copyFileSync(optimized,main);
}
report.jsScope='wasm-bindgen entry and transitively imported JS snippets; per-resource compression; excludes test loader';
fs.writeFileSync(file,JSON.stringify(report,null,2));
const byName=new Map(report.cases.map(item=>[item.name,item]));
const rows=['| Case | WASM | Brotli | JS incl. snippets | WASM Δ | Brotli Δ |','|---|---:|---:|---:|---:|---:|'];
for(const item of report.cases){const ref=byName.get(item.reference);rows.push(`| ${item.name} | ${item.optimized.bytes} | ${item.optimized.brotli} | ${item.js.bytes} | ${ref?item.optimized.bytes-ref.optimized.bytes:'—'} | ${ref?item.optimized.brotli-ref.optimized.brotli:'—'} |`);}
fs.writeFileSync(path.join(root,'summary.md'),rows.join('\n')+'\n');
console.log(`Summarized ${report.cases.length} cases including reachable JS snippets`);
