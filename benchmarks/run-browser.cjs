// Independent Chrome process; no shared MCP browser/profile. All temporary
// profiles, downloads, screenshots and raw samples belong under D:/BuildCache.
const fs=require('node:fs');
const path=require('node:path');
const vm=require('node:vm');
const assert=require('node:assert/strict');
const crypto=require('node:crypto');
const args=process.argv.slice(2);
const option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const out=path.resolve(option('--out','D:/BuildCache/glory-framework-compare/browser-round'));
if(!out.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Output must be on D:/BuildCache');
if(!(process.env.TEMP||'').replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Set TEMP/TMP on D before launching Node');
const order=option('--order','leptos,glory,dioxus').split(',');
assert.deepEqual([...order].sort(),['dioxus','glory','leptos']);
fs.mkdirSync(out,{recursive:false});
fs.mkdirSync(path.join(out,'driver-source'));
for(const name of ['run-browser.cjs','acceptance.js','browser-run.js','bench-harness.js'])fs.copyFileSync(path.join(__dirname,name),path.join(out,'driver-source',name));
const base=option('--base-url','http://127.0.0.1:8790');
const executablePath=option('--browser','C:/Program Files/Google/Chrome/Application/chrome.exe');
const {chromium}=require(path.resolve(__dirname,'../tests/playwright/node_modules/playwright'));
const parse=name=>vm.runInThisContext('('+fs.readFileSync(path.join(__dirname,name),'utf8')+')',{filename:name});
const acceptance=parse('acceptance.js'),measure=parse('browser-run.js');
const save=(name,value)=>fs.writeFileSync(path.join(out,name),JSON.stringify(value,null,2)+'\n');
const sha=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath,downloadsPath:path.join(out,'downloads')});
 const contexts=[];
 try{
  const acceptanceResults=[],results=[];
  let canonical,geometry,picture;
  for(const app of ['glory','leptos','dioxus']){
   const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1,locale:'en-US',colorScheme:'light'});
   contexts.push(context);
   const page=await context.newPage();page.setDefaultTimeout(60000);
   page.__BENCH_ARTIFACT_DIR=out;
   const errors=[];page.on('pageerror',error=>errors.push(error.message));
   await page.goto(`${base}/${app}/`);await page.bringToFront();
   const result=await acceptance(page);assert.equal(errors.length,0);
   save(`${app}-acceptance.json`,result);
   const states=result.snapshots.map(({stage,count,sha256})=>({stage,count,sha256}));
   if(canonical)assert.deepEqual(states,canonical);else canonical=states;
   if(geometry)assert.deepEqual(result.geometry,geometry);else geometry=result.geometry;
   const png=sha(path.join(out,`${app}-conformance.png`));
   if(picture)assert.equal(png,picture,'final viewport pixels must agree');else picture=png;
   acceptanceResults.push(result);await context.close();
   console.log(`Accepted ${app}: seeded data, 9 operations, keyed DOM, 10000 real boxes, cleanup, pixels`);
  }
  const outcomes=new Map();
  // Each local round uses fresh contexts and the shared untimed warmups.
  for(const app of order){
   const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1,locale:'en-US',colorScheme:'light'});
   contexts.push(context);
   const page=await context.newPage();page.setDefaultTimeout(60000);
   const errors=[];page.on('pageerror',error=>errors.push(error.message));
   await page.goto(`${base}/${app}/`);await page.bringToFront();
   const result=await measure(page);assert.equal(errors.length,0);
   save(`${app}-samples.json`,result);
   for(const row of result.results){
    assert.equal(row.samples.length,12);
    row.samples.forEach((sample,i)=>{
     const key=`${row.key}:${i}`,value=[sample.count,sample.sha256];
     if(outcomes.has(key))assert.deepEqual(value,outcomes.get(key));else outcomes.set(key,value);
    });
   }
   results.push(result);await context.close();
   console.log(`Measured ${app}: 9 x 12 samples; setup/checks outside timing; CDP CPU deltas`);
  }
  save('evidence.json',{schema:1,contract:'semantic-rows-v1',browser:browser.version(),executablePath,
   order,rounds:1,acceptanceResults,results,pixels_equal:true,
   samples_and_data_equal:true,scope:'Local full semantic DOM, click-to-two-RAF and CDP CPU counters; not official table scores or presentation/FPS/RSS'});
 }finally{
  await Promise.allSettled(contexts.map(context=>context.close()));
  await browser.close();
 }
})().catch(error=>{fs.writeFileSync(path.join(out,'failure.txt'),error.stack);console.error(error);process.exitCode=1;});
