'use strict';
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const zlib=require('node:zlib');
function javascriptMetrics(dir, entry='fixture.js') {
  const seen=new Set(),files=[];
  function visit(file) {
    if(seen.has(file)) return;
    if(!file.startsWith(path.resolve(dir)+path.sep)) throw Error('JS import escaped its case directory');
    seen.add(file);
    const bytes=fs.readFileSync(file),source=bytes.toString('utf8');
    const br=zlib.brotliCompressSync(bytes,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11,[zlib.constants.BROTLI_PARAM_MODE]:zlib.constants.BROTLI_MODE_GENERIC}});
    const gz=zlib.gzipSync(bytes,{level:9,mtime:0});
    fs.writeFileSync(file+'.br',br);fs.writeFileSync(file+'.gz',gz);
    files.push({path:path.relative(dir,file).replaceAll('\\','/'),bytes:bytes.length,brotli:br.length,gzip:gz.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
    // wasm-bindgen web output uses static relative ES module imports. Follow
    // those imports, rather than counting stale snippets left by an older run.
    for(const match of source.matchAll(/\bfrom\s*['"](\.[^'"]+\.js)['"]/g)) visit(path.resolve(path.dirname(file),match[1]));
    for(const match of source.matchAll(/\bimport\s*['"](\.[^'"]+\.js)['"]/g)) visit(path.resolve(path.dirname(file),match[1]));
  }
  visit(path.resolve(dir,entry));
  files.sort((a,b)=>a.path.localeCompare(b.path,'en'));
  return {bytes:files.reduce((n,file)=>n+file.bytes,0),brotli:files.reduce((n,file)=>n+file.brotli,0),gzip:files.reduce((n,file)=>n+file.gzip,0),sha256:crypto.createHash('sha256').update(JSON.stringify(files)).digest('hex'),files};
}
module.exports={javascriptMetrics};
