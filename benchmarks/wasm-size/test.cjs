'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {compare,enforce}=require('./compare.cjs');
const baseline=JSON.parse(fs.readFileSync(path.join(__dirname,'baseline.json')));
assert.equal(compare(baseline,baseline).failures.length,0);
const before=JSON.parse(fs.readFileSync(path.join(__dirname,'before.json')));
assert.throws(()=>enforce(compare(before,baseline).failures),/Size regression budget exceeded/);

// A shared reduction must not conceal new retained code from an unused feature.
const candidate=structuredClone(baseline);
for(const item of candidate.cases) {
  if(item.optimized.bytes>20000)item.optimized.bytes-=10000;
  if(item.optimized.brotli>20000)item.optimized.brotli-=10000;
}
const unused=candidate.cases.find(item=>item.name==='sapid-json-unused');
unused.optimized.bytes+=2048;unused.optimized.brotli+=1024;
const regression=compare(candidate,baseline).failures.find(item=>item.name===unused.name);
assert.ok(regression);
assert.ok(regression.wasm<0&&regression.brotli<0);
assert.deepEqual(regression.unusedPremiumGrowth,{wasm:2048,brotli:1024});

for(const key of ['versions','profile','compression','optimizer','verifiedTarget','lockSha256','jsScope']) {
  const incompatible=structuredClone(baseline);incompatible[key]='changed';
  assert.throws(()=>compare(incompatible,baseline),/Incompatible measurement setting/);
}
const redefined=structuredClone(baseline);redefined.cases[0].features=['base'];
assert.throws(()=>compare(redefined,baseline),/Case definition changed/);
console.log('PASS identical reports, actual pre-optimization regression, masked unused-feature growth and incompatible settings/case definitions');
