const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {chromium}=require('../../tests/playwright/node_modules/playwright');
const args=process.argv.slice(2),option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const out=path.resolve(option('--out','D:/BuildCache/glory-portfolio-20261025/run'));
assert(out.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
assert((process.env.TEMP||'').replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
fs.mkdirSync(out,{recursive:false});fs.mkdirSync(path.join(out,'driver'));
for(const file of ['compare.cjs','harness.js'])fs.copyFileSync(path.join(__dirname,file),path.join(out,'driver',file));
const variants=['glory','dioxus-root','dioxus-row'],sizes=option('--sizes','1000,5000').split(',').map(Number);
const base=option('--base','http://127.0.0.1:29045'),pin=option('--pin','D:/BuildCache/glory-portfolio-20261025/pin-browser.ps1');
const operations=['mount','one','sparse','medium','dense','burst','select','clear'];
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const save=(file,value)=>fs.writeFileSync(path.join(out,file+'.json'),JSON.stringify(value,null,2)+'\n');
const evidence={contract:'portfolio-v1',warmup:3,samples:12,acceptance:[],rounds:[],timing:'CDP script/task/layout/style deltas; wall click to two RAF; checks excluded'};
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',downloadsPath:path.join(out,'downloads')});
 evidence.browser=browser.version();
 async function withPage(variant,n,callback){
  const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1,locale:'en-US',colorScheme:'light'});
  try{const page=await context.newPage();page.setDefaultTimeout(120000);
   await page.goto(`${base}/site/${variant}/?n=${n}`);await page.bringToFront();await page.waitForFunction(()=>window.scenarioReady);
   await page.addScriptTag({path:path.join(__dirname,'harness.js')});await page.evaluate(()=>document.fonts.ready);
   require('node:child_process').execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-File',pin,'-ProfileRoot',process.env.TEMP,'-Out',path.join(out,'cpu-affinity.jsonl')]);
   return await callback(page);
  } finally{await context.close();}
 }
 try{
  for(const n of sizes){let canonical;
   for(const variant of variants){const accepted=await withPage(variant,n,async p=>{
    const baseline=await p.evaluate(()=>window.scenario.baseline()),steps=[],pictures=[];
    for(const op of ['mount','one','sparse','medium','dense','burst','select']){
     const state=await p.evaluate(op=>window.scenario.run(op),op);steps.push({op,count:state.count,sha256:state.sha256});
     if(['mount','dense','select'].includes(op)){
      if(op==='select')await p.evaluate(async()=>{document.querySelector('.viewport').scrollTop=97*28;await window.scenario.settle();});
      const png=await p.screenshot({path:path.join(out,`${variant}-${n}-${op}.png`)});pictures.push({op,sha256:sha(png)});
     }
    }
    const geometry=await p.evaluate(()=>window.scenario.geometry()),summaryGeometry=await p.evaluate(()=>window.scenario.summaryGeometry());
    assert.equal(summaryGeometry.numeric,'tabular-nums');
    assert.equal(geometry.length,n);assert(geometry.every(row=>row.every(Number.isFinite)&&row[2]>0&&row[3]===28));
    const cleared=await p.evaluate(()=>window.scenario.run('clear'));
    assert.deepEqual(cleared.stats,baseline);
    const png=await p.screenshot({path:path.join(out,`${variant}-${n}-clear.png`)});pictures.push({op:'clear',sha256:sha(png)});
    return{steps,pictures,geometry_sha256:sha(JSON.stringify(geometry)),summaryGeometry,baseline,cleared};
   });
   const comparable={steps:accepted.steps,pictures:accepted.pictures,geometry:accepted.geometry_sha256,summaryGeometry:accepted.summaryGeometry};
   if(canonical)assert.deepEqual(comparable,canonical);else canonical=comparable;
   evidence.acceptance.push({variant,n,...accepted});save('evidence',evidence);console.log('Accepted',variant,n);
   }
  }
  if(args.includes('--acceptance-only'))return;
  const outcomes=new Map();
  for(let round=0;round<2;round++)for(const n of sizes)for(const variant of round%2?[...variants].reverse():variants){
   const result=await withPage(variant,n,async p=>{
    const cdp=await p.context().newCDPSession(p);await cdp.send('Performance.enable');const results=[];
    const metrics=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(x=>[x.name,x.value]));
    const run=(op,identity=true)=>p.evaluate(({op,identity})=>window.scenario.run(op,identity),{op,identity});
    for(const op of operations){
     await p.evaluate(()=>window.scenario.release());await run('clear',false);
     if(op!=='mount'&&op!=='clear')await run('mount');
     const samples=[];
     for(let i=-3;i<12;i++){
      if(op==='mount')await run('clear',false);
      if(op==='clear'){await run('mount',false);await p.evaluate(()=>window.scenario.release());}
      const before=i>=0?await metrics():null;
      const wall=await p.evaluate(op=>window.scenario.click(op),op);
      const after=i>=0?await metrics():null;
      const state=await p.evaluate(({op,identity})=>window.scenario.verify(op,identity),{op,identity:op!=='mount'&&op!=='clear'});
      if(i>=0){const counters=Object.fromEntries(['ScriptDuration','TaskDuration','LayoutDuration','RecalcStyleDuration'].map(k=>[k,(after[k]-before[k])*1000]));
       const sample={wall_ms:wall,...counters,...state};samples.push(sample);
       const key=`${round}:${n}:${op}:${i}`,value=[state.count,state.sha256];if(outcomes.has(key))assert.deepEqual(value,outcomes.get(key));else outcomes.set(key,value);
      }
     }
     results.push({op,samples});
    }
    await p.evaluate(()=>window.scenario.release());const final=await run('clear',false);await cdp.detach();return{results,final};
   });
   evidence.rounds.push({round,n,variant,...result});save(`round-${round}-${n}-${variant}`,result);save('evidence',evidence);console.log('Measured',round,n,variant);
  }
 }finally{save('evidence',evidence);await browser.close();}
})().catch(error=>{fs.writeFileSync(path.join(out,'failure.txt'),error.stack);console.error(error);process.exitCode=1;});
