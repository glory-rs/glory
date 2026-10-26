// Attribute final function bodies using a name-preserving Binaryen output.
// Usage: node attribute.cjs named.wasm optimized.wasm > D:/BuildCache/.../attribution.json
'use strict';
const fs=require('node:fs'),crypto=require('node:crypto');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function inspect(file) {
  const bytes=fs.readFileSync(file);let offset=8,imported=0;
  if(!bytes.subarray(0,8).equals(Buffer.from([0,97,115,109,1,0,0,0])))throw Error('Expected core WASM v1');
  const sections=[],names=new Map(),bodies=[],data=[];
  function leb(){let n=0,s=0,b;do{b=bytes[offset++];n+=(b&127)*2**s;s+=7;}while(b&128);return n;}
  function string(){const n=leb(),s=bytes.toString('utf8',offset,offset+n);offset+=n;return s;}
  imported=WebAssembly.Module.imports(new WebAssembly.Module(bytes)).filter(item=>item.kind==='function').length;
  while(offset<bytes.length){
    const start=offset,id=bytes[offset++],size=leb(),payload=offset,end=offset+size;
    const section={id,payloadBytes:size,encodedBytes:end-start,sha256:hash(bytes.subarray(payload,end))};
    if(id===0){
      section.name=string();
      if(section.name==='name')while(offset<end){
        const sub=bytes[offset++],n=leb(),subEnd=offset+n;
        if(sub===1){const count=leb();for(let i=0;i<count;i++)names.set(leb(),string());}
        offset=subEnd;
      }
    }else if(id===10){
      const count=leb();for(let i=0;i<count;i++){
        const n=leb();bodies.push({index:imported+i,bytes:n});offset+=n;
      }
    }else if(id===11){
      const count=leb();for(let i=0;i<count;i++){
        const flags=leb();
        if(flags===2)leb();else if(flags!==0&&flags!==1)throw Error('Unsupported data segment');
        if(flags!==1){
          const opcode=bytes[offset++];
          if(opcode!==0x41&&opcode!==0x23)throw Error('Unsupported data offset');
          leb();if(bytes[offset++]!==0x0b)throw Error('Unsupported constant expression');
        }
        const n=leb();data.push(bytes.subarray(offset,offset+n));offset+=n;
      }
    }
    sections.push(section);offset=end;
  }
  return {file,bytes:bytes.length,sha256:hash(bytes),sections,data,bodies:bodies.map(f=>({...f,name:(names.get(f.index)||`function ${f.index}`).replace(/\[[a-f0-9]{8,}\]/g,'')}))};
}
function owner(name){return name.match(/(?:^|[^\w])([a-zA-Z_]\w*)::/)?.[1]||'generated';}
function group(functions,key) {
  const buckets=new Map();for(const f of functions){const k=key(f);const b=buckets.get(k)||{name:k,bytes:0,functions:0};b.bytes+=f.bytes;b.functions++;buckets.set(k,b);}
  return [...buckets.values()].sort((a,b)=>b.bytes-a.bytes);
}
const named=inspect(process.argv[2]),final=inspect(process.argv[3]);
const nonCustom=module=>module.sections.filter(s=>s.id!==0).map(({id,sha256})=>({id,sha256}));
if(JSON.stringify(nonCustom(named))!==JSON.stringify(nonCustom(final)))throw Error('Named code/data differ from measured final WASM');
const functions=named.bodies.sort((a,b)=>b.bytes-a.bytes),functionBytes=functions.reduce((n,f)=>n+f.bytes,0);
const ascii=final.data.flatMap(segment=>[...segment.toString('latin1').matchAll(/[\x20-\x7e]{8,}/g)].map(m=>m[0]));
const result={schema:1,final:{path:final.file,bytes:final.bytes,sha256:final.sha256},nonCustomSectionsIdentical:true,
  attribution:'Non-overlapping function bodies grouped by enclosing symbol; inlined callees are charged to the enclosing symbol. These are not exclusive crate sizes or removable-size estimates.',
  sections:final.sections,functionBytes,functionCount:functions.length,
  enclosingSymbols:group(functions,f=>owner(f.name)),
  structuralBodies:group(functions,f=>{
    if(/^core::ptr::(?:drop_glue|drop_in_place)/.test(f.name))return 'destruction glue';
    if(/^<?alloc::collections::btree::/.test(f.name))return 'BTree collection implementations';
    if(owner(f.name)==='hashbrown')return 'hash collection implementations';
    if(f.name.includes('::fmt::')||f.name.includes('::flt2dec::')||f.name.includes('::dec2flt::'))return 'formatting / number conversion symbols';
    if(owner(f.name)==='dlmalloc')return 'dlmalloc implementation';
    if(f.name.includes(' as core::clone::Clone>'))return 'Clone implementations';
    return 'remaining bodies';
  }),
  webModules:group(functions.filter(f=>owner(f.name)==='glory_web'),f=>f.name.match(/glory_web::browser::([a-z_]+)::(?!<)/)?.[1]||'browser entry / Renderer'),
  regularExpressionSymbols:functions.filter(f=>/(?:^|[^\w])(?:regex|regex_automata|regex_syntax|aho_corasick|fancy_regex)::/.test(f.name)),
  data:{segments:final.data.length,bytes:final.data.reduce((n,b)=>n+b.length,0),asciiRunBytes:ascii.reduce((n,s)=>n+s.length,0),longestAsciiRuns:ascii.sort((a,b)=>b.length-a.length).slice(0,12).map(s=>({bytes:s.length,preview:s.slice(0,400)}))},
  largestFunctions:functions.slice(0,35)};
process.stdout.write(JSON.stringify(result,null,2)+'\n');
