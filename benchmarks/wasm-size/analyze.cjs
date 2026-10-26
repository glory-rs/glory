// Report actual uncompressed function bodies in a name-bearing bindgen WASM.
'use strict';
const fs=require('node:fs');
const bytes=fs.readFileSync(process.argv[2]); let offset=8;
function leb(){let n=0,s=0,b;do{b=bytes[offset++];n+=(b&127)*2**s;s+=7;}while(b&128);return n;}
function string(){const n=leb(),s=bytes.toString('utf8',offset,offset+n);offset+=n;return s;}
const names=new Map(), functions=[];
const imported=WebAssembly.Module.imports(new WebAssembly.Module(bytes)).filter(entry=>entry.kind==='function').length;
while(offset<bytes.length){const id=bytes[offset++],size=leb(),end=offset+size;
  if(id===0&&string()==='name'){
    while(offset<end){const sub=bytes[offset++],subSize=leb(),subEnd=offset+subSize;if(sub===1){const count=leb();for(let i=0;i<count;i++){const index=leb();names.set(index,string());}}offset=subEnd;}
  }else if(id===10){const count=leb();for(let i=0;i<count;i++){const size=leb();functions.push({index:imported+i,bytes:size});offset+=size;}}
  offset=end;
}
const result=functions.map(fn=>({...fn,name:names.get(fn.index)||`function ${fn.index}`})).sort((a,b)=>b.bytes-a.bytes);
const limit=Number(process.argv[3]||60);
process.stdout.write(JSON.stringify(result.slice(0,limit),null,2)+'\n');
