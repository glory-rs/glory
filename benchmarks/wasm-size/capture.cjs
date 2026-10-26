// Explicitly promote a complete, browser-verified run to a reviewed JSON baseline.
'use strict';
const fs=require('node:fs'),path=require('node:path');
const root=path.resolve(process.argv[2]),destination=path.resolve(process.argv[3]);
const report=JSON.parse(fs.readFileSync(path.join(root,'report.json')));
const browser=JSON.parse(fs.readFileSync(path.join(root,'browser.json')));
const sources=JSON.parse(fs.readFileSync(path.join(root,'sources.json')));
const matrix=JSON.parse(fs.readFileSync(path.join(root,'source/glory/benchmarks/wasm-size/cases.json')));
if(report.cases.length!==matrix.length||!matrix.every(item=>report.cases.some(entry=>JSON.stringify(entry.features)===JSON.stringify(item.features)&&entry.name===item.name)))throw Error('Incomplete matrix');
if(browser.sourceDigest!==report.sourceDigest||sources.digest!==report.sourceDigest||!report.cases.every(item=>browser.cases.some(entry=>entry.name===item.name&&entry.sha256===item.optimized.sha256&&entry.passed)))throw Error('Missing matching browser evidence');
const result={...report,recorded:new Date().toISOString().slice(0,10),inputSource:{gloryRevision:sources.gloryRevision,sapidRevision:sources.sapidRevision,gloryStatus:sources.gloryStatus,sapidStatus:sources.sapidStatus},
  browser:{version:browser.browser,passed:report.cases.length},cases:report.cases.map(({buildMs,rustcFresh,...item})=>item)};
fs.writeFileSync(destination,JSON.stringify(result,null,2)+'\n');
console.log(`Captured ${report.cases.length} browser-verified cases: ${destination}`);
