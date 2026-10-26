'use strict';
const fs=require('node:fs');
function compare(report,previous) {
  for(const key of ['versions','profile','compression','optimizer','verifiedTarget','lockSha256','jsScope']) {
    if(JSON.stringify(report[key])!==JSON.stringify(previous[key])) throw Error(`Incompatible measurement setting: ${key}`);
  }
  const lookup=new Map(report.cases.map(item=>[item.name,item]));
  const oldCases=new Map(previous.cases.map(item=>[item.name,item]));
  const comparison=report.cases.map(item=>{
    const old=oldCases.get(item.name);
    if(!old) throw Error(`Comparison is missing ${item.name}`);
    if(JSON.stringify(item.features)!==JSON.stringify(old.features)||item.reference!==old.reference)throw Error(`Case definition changed: ${item.name}`);
    const ref=lookup.get(item.reference),oldRef=oldCases.get(item.reference);
    const unused=item.name.includes('unused')&&ref&&oldRef;
    return {name:item.name,wasm:item.optimized.bytes-old.optimized.bytes,brotli:item.optimized.brotli-old.optimized.brotli,js:item.js.bytes-old.js.bytes,
      unusedPremiumGrowth:unused?{wasm:(item.optimized.bytes-ref.optimized.bytes)-(old.optimized.bytes-oldRef.optimized.bytes),brotli:(item.optimized.brotli-ref.optimized.brotli)-(old.optimized.brotli-oldRef.optimized.brotli)}:null};
  });
  const failures=comparison.filter(item=>{const old=oldCases.get(item.name);return item.wasm>Math.max(1024,old.optimized.bytes*0.005)||item.brotli>Math.max(512,old.optimized.brotli*0.005)||item.js>Math.max(512,old.js.bytes*0.005)||(item.unusedPremiumGrowth&&(item.unusedPremiumGrowth.wasm>1024||item.unusedPremiumGrowth.brotli>512));});
  return {comparison,failures};
}
function enforce(failures) {
  if(failures.length) throw Error(`Size regression budget exceeded (0.5% totals with 1 KiB WASM / 512 B Brotli or JS floors; unused premiums 1 KiB / 512 B): ${failures.map(item=>item.name).join(', ')}`);
}
module.exports={compare,enforce};
if(require.main===module) {
  const result=compare(JSON.parse(fs.readFileSync(process.argv[2])),JSON.parse(fs.readFileSync(process.argv[3])));
  console.log(JSON.stringify(result,null,2));
  if(process.argv.includes('--check'))enforce(result.failures);
}
