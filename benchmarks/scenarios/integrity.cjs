// Supplemental ordered, per-node identity checks, separate from all timings.
const {chromium}=require('../../tests/playwright/node_modules/playwright');
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const args=process.argv.slice(2),get=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const out=path.resolve(get('--out','D:/BuildCache/glory-portfolio-20261025/integrity.json'));
assert(out.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
assert((process.env.TEMP||'').replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
(async()=>{const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});const records=[];
try{for(const n of [1000,5000])for(const variant of ['glory','dioxus-root','dioxus-row']){
 const context=await browser.newContext();try{const p=await context.newPage();await p.goto(`${get('--base','http://127.0.0.1:29045')}/site/${variant}/?n=${n}`);await p.waitForFunction(()=>window.scenarioReady);await p.addScriptTag({path:path.join(__dirname,'harness.js')});
 const result=await p.evaluate(async()=>{
  const collect=()=>[...document.querySelectorAll('.quotes > .quote')].flatMap(row=>{const a=[row],w=document.createTreeWalker(row,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT);while(w.nextNode())a.push(w.currentNode);return a;});
  await window.scenario.run('mount');const before=collect(),ids=before.map(n=>n.nodeType===1?n.getAttribute('data-gly-node'):null);const steps=[];
  for(const op of ['one','sparse','medium','dense','burst','select']){const state=await window.scenario.run(op),after=collect();if(after.length!==before.length||after.some((node,i)=>node!==before[i]))throw Error('ordered identity changed '+op);if(after.some((node,i)=>node.nodeType===1&&node.getAttribute('data-gly-node')!==ids[i]))throw Error('semantic identity changed');steps.push({op,sha256:state.sha256});}
  await window.scenario.run('clear');if(before.some(n=>n.isConnected))throw Error('old nodes connected after close');
  await window.scenario.run('mount');const fresh=collect(),old=new Set(before),oldIds=new Set(ids.filter(Boolean));
  if(fresh.some(n=>old.has(n)))throw Error('reused retired DOM object');
  if(location.pathname.includes('glory')&&fresh.some(n=>n.nodeType===1&&(!/^\d+\.\d+\.\d+$/.test(n.getAttribute('data-gly-node'))||oldIds.has(n.getAttribute('data-gly-node')))))throw Error('reused/invalid semantic lifetime');
  await window.scenario.run('clear');return{physicalNodes:before.length,orderedIdentityUpdates:steps.length,steps,retiredAndRemounted:true,final:window.scenarioStats()};
 });records.push({n,variant,...result});console.log('Identity accepted',variant,n,result.physicalNodes);
 }finally{await context.close();}
}}finally{await browser.close();}fs.writeFileSync(out,JSON.stringify({scope:'Separate from timing; every ordered Element/Text, full Glory ID, retired/remount lifetime',records},null,2));})().catch(e=>{console.error(e);process.exitCode=1;});
