'use strict';
const fs = require('node:fs'), path = require('node:path'), http = require('node:http');
const assert = require('node:assert/strict'), crypto = require('node:crypto'), vm = require('node:vm');
const cp = require('node:child_process');
const root = path.resolve(process.env.GLORY_BENCH_BUILD_ROOT || 'D:/BuildCache/glory-benchmark-dsl-20261028'), repo = process.env.GLORY_REPO || 'D:/Works/glory-rs/glory';
const out = path.join(root, process.argv[2] || 'trial1');
const rounds = Number(process.argv[3] || 4);
const variants = (process.argv[4] || 'before,glory,handwritten,dioxus').split(',');
const routes = {before:"before/site/glory",glory:"after/site/glory",handwritten:"handwritten/site/glory",probes:"probes/site/glory",handwritten_probes:"handwritten-probes/site/glory",dioxus:"before/site/dioxus",leptos:"before/site/leptos",api:"first-api/site/glory",fields:"direct-fields/site/glory",arena:"arena/site/glory"};
if(process.env.GLORY_CANDIDATE_SITE)routes.glory=process.env.GLORY_CANDIDATE_SITE;
if(process.env.GLORY_PROBES_SITE)routes.probes=process.env.GLORY_PROBES_SITE;
if(process.env.GLORY_HANDWRITTEN_SITE)routes.handwritten=process.env.GLORY_HANDWRITTEN_SITE;
routes.completed='completed/site/glory';
const { chromium } = require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
process.env.TEMP = process.env.TMP = path.join(root, 'tmp');
fs.mkdirSync(out);
fs.mkdirSync(path.join(out, 'driver-source'));
for (const name of ['acceptance.js', 'browser-run.js', 'bulk-integrity.js', 'bench-harness.js', 'display-acceptance.js', 'scenarios/pin-browser.ps1']) {
  fs.copyFileSync(path.join(repo, 'benchmarks', name), path.join(out, 'driver-source', path.basename(name)));
}
fs.copyFileSync(__filename, path.join(out, 'driver-source/compare.cjs'));
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const parse = file => vm.runInThisContext('(' + fs.readFileSync(path.join(out, 'driver-source', file), 'utf8') + ')');
const acceptance = parse('acceptance.js'), measure = parse('browser-run.js');
const sparseAcceptance = vm.runInThisContext('(' + fs.readFileSync(path.join(out,'driver-source/display-acceptance.js'),'utf8') + ')');
const integrity = parse("bulk-integrity.js");
const environment = () => JSON.parse(cp.execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
  "[pscustomobject]@{time=(Get-Date).ToUniversalTime().ToString('o');load=(Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor | Where-Object Name -eq _Total).PercentProcessorTime;builds=@(Get-Process -Name cargo,rustc -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU)} | ConvertTo-Json -Depth 4 -Compress"
], { encoding:'utf8', windowsHide:true, timeout:30000 }));
const evidence = { schema:1, contract:'semantic-rows-v1', profiles:{before:"xml-display-v1",glory:"xml-display-keyed-v2",handwritten:"named-display-keyed-v2",api:"first-keyed-api",dioxus:"dioxus-0.7.10",leptos:"leptos-0.8.22"}, baseline_source:JSON.parse(fs.readFileSync(path.join(root,'before/inputs/inputs.json'))), source:JSON.parse(fs.readFileSync(process.env.GLORY_CANDIDATE_INPUTS||path.join(root,'after/inputs/inputs.json'))),
  launch:{ headless:true, args:['--disable-gpu'], viewport:{width:1280,height:900}, deviceScaleFactor:1, locale:'en-US', colorScheme:'light', fresh_browser_per_variant:true },
  acceptance:[], rounds:[], artifacts:Object.fromEntries(variants.map(name => [name, {
    wasm_bytes:fs.statSync(path.join(root,routes[name],'app_bg.wasm')).size,
    wasm_sha256:sha(path.join(root,routes[name],'app_bg.wasm')),
    js_bytes:fs.readdirSync(path.join(root,routes[name]),{recursive:true}).filter(file=>file.endsWith('.js') && file!=='bench-harness.js').reduce((n,file)=>n+fs.statSync(path.join(root,routes[name],file)).size,0)
  }])) };
const save = () => fs.writeFileSync(path.join(out,'evidence.json'), JSON.stringify(evidence,null,2)+'\n');
const server = http.createServer((req,res) => {
  try {
    let file=path.resolve(root,'.'+new URL(req.url,'http://localhost').pathname);
    assert(file.startsWith(root+path.sep));
    if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');
    res.setHeader('Cache-Control','no-store');
    res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');
    fs.createReadStream(file).pipe(res);
  } catch {res.statusCode=404;res.end();}
});
async function withPage(name,callback) {
  const browser=await chromium.launch({headless:true,args:['--disable-gpu'],executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',downloadsPath:path.join(out,'downloads')});
  try {
    evidence.browser=browser.version();
    const context=await browser.newContext({viewport:evidence.launch.viewport,deviceScaleFactor:1,locale:'en-US',colorScheme:'light'});
    const page=await context.newPage();page.setDefaultTimeout(120000);page.__BENCH_ARTIFACT_DIR=out;
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/${routes[name]}/`);await page.bringToFront();
    await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
    cp.execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-File',path.join(out,'driver-source/pin-browser.ps1'),'-ProfileRoot',process.env.TEMP,'-Out',path.join(out,'affinity.jsonl')],{windowsHide:true,timeout:30000});
    const result=await callback(page);assert.deepEqual(errors,[]);return result;
  } finally {await browser.close();}
}
(async()=>{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  try {
    let canonical,pixels,fullGeometry;
    for(const name of variants) {
      const result=await withPage(name,async page=>{
        const accepted=await acceptance(page);
        const data=accepted.snapshots.map(({stage,count,sha256})=>({stage,count,sha256}));
        if(canonical)assert.deepEqual(data,canonical);else canonical=data;
        await page.reload();await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
        const identities=await integrity(page);
        const geometryAndData={steps:identities.steps,geometry:identities.geometry};
        if(fullGeometry)assert.deepEqual(geometryAndData,fullGeometry);else fullGeometry=geometryAndData;
        await page.reload();await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
        const pictures=[];
        for(const [stage,selector] of [['create','.bench-run'],['replace','.bench-run'],['append','.bench-add'],['tenk','.bench-runlots'],['clear','.bench-clear']]) {
          await page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},selector);
          const file=path.join(out,`${name}-${stage}.png`);await page.screenshot({path:file,timeout:120000});pictures.push({stage,sha256:sha(file)});
          if(stage==='tenk') {
            await page.evaluate(async()=>{window.scrollTo(0,document.querySelectorAll('.bench-row')[5000].offsetTop);await window.__BENCH__.settle();});
            const middle=path.join(out,`${name}-middle.png`);await page.screenshot({path:middle,timeout:120000});pictures.push({stage:'middle',sha256:sha(middle)});
            await page.evaluate(async()=>{window.scrollTo(0,0);await window.__BENCH__.settle();});
          }
        }
        if(pixels)assert.deepEqual(pictures,pixels);else pixels=pictures;
        const sparse = ['probes','handwritten_probes'].includes(name) ? await sparseAcceptance(page) : null;
        return {name,...accepted,identities,pictures,sparse};
      });
      evidence.acceptance.push(result);save();console.log('Accepted '+name+': data, full descendant identities, 10k rectangles, six matching screenshots');
    }
    const outcomes=new Map();
    for(let round=0;round<rounds;round++)for(const name of round%2?[...variants].reverse():variants) {
      const run=await withPage(name,async page=>{
        const before=environment();assert.equal(before.builds.length,0,'Stop owned builds before measuring');
        const result=await measure(page),after=environment();assert.equal(after.builds.length,0);
        return {round,name,before,after,...result};
      });
      assert.deepEqual(run.errors,[]);assert.equal(run.warmup,3);assert.equal(run.samples,12);assert.equal(run.results.length,9);
      for(const op of run.results) {assert.equal(op.samples.length,12);op.samples.forEach((sample,i)=>{
        const key=op.key+':'+i,value=[sample.count,sample.sha256];if(outcomes.has(key))assert.deepEqual(value,outcomes.get(key));else outcomes.set(key,value);
      });}
      evidence.rounds.push(run);save();console.log('Measured round '+(round+1)+'/'+rounds+': '+name);
    }
    for(const [file,digest] of Object.entries(evidence.source.sha256))assert.equal(sha(path.join(evidence.source.snapshot,file)),digest,'Frozen input changed: '+file);
    for(const [file,digest] of Object.entries(evidence.baseline_source.sha256))assert.equal(sha(path.join(root,'before/inputs',file)),digest,'Frozen baseline changed: '+file);
    evidence.completed=new Date().toISOString();save();
  }catch(error){evidence.failure=String(error.stack);save();throw error;}
  finally{server.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
