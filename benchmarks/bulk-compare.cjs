// Compare independently built Glory revisions with the unchanged rows harness.
// Example: node benchmarks/bulk-compare.cjs --base http://127.0.0.1:8791 --out D:/BuildCache/glory-bulk-optimization/round
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const assert = require('node:assert/strict'), crypto = require('node:crypto');
const args = process.argv.slice(2);
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
const out = path.resolve(option('--out', 'D:/BuildCache/glory-bulk-optimization/round'));
assert(out.replaceAll('\\', '/').toLowerCase().startsWith('d:/buildcache/'));
assert((process.env.TEMP || '').replaceAll('\\', '/').toLowerCase().startsWith('d:/buildcache/'), 'Set TEMP/TMP on D before launching Node');
fs.mkdirSync(out, {recursive:false});
fs.mkdirSync(path.join(out,'driver-source'));
for(const name of ['bulk-compare.cjs','acceptance.js','browser-run.js','bench-harness.js','bulk-integrity.js']) {
 fs.copyFileSync(path.join(__dirname,name),path.join(out,'driver-source',name));
}
const base = option('--base', 'http://127.0.0.1:8791');
const variants = option('--variants', 'before,release').split(',');
const probeOnly = args.includes('--probe');
const {chromium} = require('../tests/playwright/node_modules/playwright');
const parse = name => vm.runInThisContext('(' + fs.readFileSync(path.join(__dirname, name), 'utf8') + ')');
const acceptance = parse('acceptance.js'), measure = parse('browser-run.js');
const save = (name, value) => fs.writeFileSync(path.join(out, name + '.json'), JSON.stringify(value, null, 2) + '\n');
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
async function census(page) {
 return page.evaluate(() => {
  const root = document.getElementById('main'), walker = document.createTreeWalker(root, NodeFilter.SHOW_ALL);
  const result = {elements:0,text:0,comments:0,attributes:0,stats:window.__BENCH_STATS__()};
  while(walker.nextNode()) {const node=walker.currentNode; if(node.nodeType===1){result.elements++;result.attributes+=node.attributes.length;}else if(node.nodeType===3)result.text++;else if(node.nodeType===8)result.comments++;}
  return result;
 });
}
async function probe(page, cdp, variant) {
 await page.evaluate(() => {
  const counts={}, time={}, attrs={};
  const wrap = (proto, name) => {const original=proto[name];proto[name]=function(...args){const start=performance.now();try{return original.apply(this,args);}finally{counts[name]=(counts[name]||0)+1;time[name]=(time[name]||0)+performance.now()-start;if(name==='setAttribute')attrs[args[0]]=(attrs[args[0]]||0)+1;}};};
  for(const name of ['createElement','createTextNode','createComment','createDocumentFragment'])wrap(Document.prototype,name);
  wrap(Element.prototype,'setAttribute');
  for(const name of ['appendChild','insertBefore','removeChild'])wrap(Node.prototype,name);
  const active=Object.getOwnPropertyDescriptor(Document.prototype,'activeElement');
  Object.defineProperty(Document.prototype,'activeElement',{...active,get(){counts.activeElement=(counts.activeElement||0)+1;return active.get.call(this);}});
  window.__DOM_PROBE__={reset(){for(const object of [counts,time,attrs])for(const key of Object.keys(object))delete object[key];},read(){return {counts:{...counts},inclusive_ms:{...time},attributes:{...attrs}};}};
 });
 const results=[];
 for(const [name,setup,action] of [['create_1k','.bench-clear','.bench-run'],['replace_1k','.bench-run','.bench-run'],['clear_1k','.bench-run','.bench-clear'],['create_10k','.bench-clear','.bench-runlots'],['clear_10k',null,'.bench-clear'],['clear_1k_after_10k','.bench-run','.bench-clear']]) {
  if(setup)await page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},setup);
  await cdp.send('Profiler.enable');await cdp.send('Profiler.setSamplingInterval',{interval:100});
  await page.evaluate(()=>window.__DOM_PROBE__.reset());await cdp.send('Profiler.start');
  await page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},action);
  const {profile}=await cdp.send('Profiler.stop');save(`${variant}-${name}-cpu`,profile);
  const counts=await page.evaluate(()=>window.__DOM_PROBE__.read()),dom=await census(page);
  const nodes=new Map(profile.nodes.map(node=>[node.id,node]));const weights=new Map();
  for(let i=0;i<(profile.samples||[]).length;i++){const id=profile.samples[i];weights.set(id,(weights.get(id)||0)+(profile.timeDeltas[i]||0));}
  const top=[...weights].map(([id,us])=>({ms:us/1000,...nodes.get(id).callFrame})).sort((a,b)=>b.ms-a.ms).slice(0,45);
  results.push({name,counts,dom,top});
 }
 return results;
}
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:option('--browser','C:/Program Files/Google/Chrome/Application/chrome.exe'),downloadsPath:path.join(out,'downloads')});
 const contextOptions={viewport:{width:1280,height:900},deviceScaleFactor:1,locale:'en-US',colorScheme:'light'};
 const evidence={schema:1,contract:'semantic-rows-v1',browser:browser.version(),probe_only:probeOnly,acceptance:[],rounds:[],probes:[],driver_sha256:sha(__filename),harness_sha256:sha(path.join(__dirname,'bench-harness.js'))};
async function withPage(variant,callback){const context=await browser.newContext(contextOptions);try{const page=await context.newPage();page.__BENCH_ARTIFACT_DIR=out;page.setDefaultTimeout(120000);await page.goto(`${base}/${variant}/site/glory/`);await page.bringToFront();await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);return await callback(page);}finally{await context.close();}}
 try {
  if(args.includes('--integrity')) {
   let canonical;
   evidence.integrity=[];
   for(const variant of variants){
    const result=await withPage(variant,parse('bulk-integrity.js'));
    if(canonical)assert.deepEqual(result,canonical);else canonical=result;
    evidence.integrity.push({variant,result});save(variant+'-integrity',result);
    console.log('Accepted '+variant+': surviving elements/text/semantic IDs and all 10000 row rectangles');
   }
   save('evidence',evidence);return;
  }
  if(!probeOnly) {
   let canonical,pixels;
   for(const variant of variants) {
    const result=await withPage(variant,async page=>{
     const accepted=await acceptance(page);
     const data=accepted.snapshots.map(({stage,count,sha256})=>({stage,count,sha256}));
     if(canonical)assert.deepEqual(data,canonical);else canonical=data;
     // Same fresh state/seed for every revision; structural operations and deep scroll.
     await page.reload();await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
     const pictures=[];
     for(const [name,selector] of [['create','.bench-run'],['replace','.bench-run'],['append','.bench-add'],['tenk','.bench-runlots'],['clear','.bench-clear']]) {
      await page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},selector);
      const file=path.join(out,`${variant}-${name}.png`);await page.screenshot({path:file});pictures.push({name,sha256:sha(file)});
      if(name==='tenk') {
       await page.evaluate(()=>window.scrollTo(0,document.querySelectorAll('.bench-row')[5000].offsetTop));await page.evaluate(()=>window.__BENCH__.settle());
       const middle=path.join(out,`${variant}-middle.png`);await page.screenshot({path:middle});pictures.push({name:'middle',sha256:sha(middle)});
       await page.evaluate(()=>window.scrollTo(0,0));
      }
     }
     if(pixels)assert.deepEqual(pictures,pixels);else pixels=pictures;
     return {variant,...accepted,pictures};
    });
    evidence.acceptance.push(result);save(variant+'-acceptance',result);console.log('Accepted '+variant+': data, keyed DOM, teardown, six viewport pixel checks');
   }
   const outcomes=new Map();
   // Reverse order in the second round. Each variant has its own fresh context.
   for(let round=0;round<Number(option('--rounds','2'));round++)for(const variant of round%2===0?variants:[...variants].reverse()) {
    const result=await withPage(variant,measure);
    for(const row of result.results)row.samples.forEach((sample,i)=>{const key=`${row.key}:${i}`,value=[sample.count,sample.sha256];if(outcomes.has(key))assert.deepEqual(value,outcomes.get(key));else outcomes.set(key,value);});
    evidence.rounds.push({round,variant,...result});save(`${round}-${variant}-samples`,result);console.log(`Measured round ${round+1}: ${variant}`);
   }
  }
  for(const variant of variants) {
   const result=await withPage(variant,async page=>{const cdp=await page.context().newCDPSession(page);try{return await probe(page,cdp,variant);}finally{await cdp.detach();}});
   evidence.probes.push({variant,results:result});save(variant+'-probe',result);console.log('Profiled '+variant+' separately; diagnostic overhead excluded from timings');
  }
  save('evidence',evidence);
 } finally {await browser.close();}
})().catch(error=>{fs.writeFileSync(path.join(out,'failure.txt'),error.stack);console.error(error);process.exitCode=1;});
