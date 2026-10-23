// Diagnostic only: compare a fresh 1000-row selection with selection after
// repeated replacement. Not pooled into the nine-operation timing table.
const {chromium}=require('../tests/playwright/node_modules/playwright');
const fs=require('node:fs');
const path=require('node:path');
const args=process.argv.slice(2);
const option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const out=path.resolve(option('--out',args[0]||'D:/BuildCache/glory-framework-compare/selection-probe'));
const base=option('--base-url','http://127.0.0.1:8790');
if(!out.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Use D:/BuildCache');
if(!(process.env.TEMP||'').replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Set TEMP/TMP on D');
fs.mkdirSync(out,{recursive:false});
fs.copyFileSync(__filename,path.join(out,'selection-probe.cjs'));
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
 const results=[];
 try{
  for(const app of ['leptos','glory','dioxus'])for(const replacements of [1,30]){
   const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1});
   try{
    const page=await context.newPage();await page.goto(`${base}/${app}/`);
    await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
    await page.evaluate(async count=>{for(let i=0;i<count;i++){document.querySelector('.bench-run').click();await window.__BENCH__.settle();}},replacements);
    const cdp=await context.newCDPSession(page);await cdp.send('Performance.enable');
    const counter=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m=>[m.name,m.value]));
    const samples=[];
    for(let i=0;i<15;i++){
     const before=await counter();
     const elapsed_ms=await page.evaluate(async index=>{const start=performance.now();document.querySelectorAll('.bench-row .lbl')[index%2].click();await window.__BENCH__.settle();return performance.now()-start;},i);
     const after=await counter();
     if(i>=3)samples.push({elapsed_ms,script_ms:(after.ScriptDuration-before.ScriptDuration)*1000});
    }
    const result={app,replacements,samples};results.push(result);
    fs.writeFileSync(path.join(out,`${app}-${replacements}.json`),JSON.stringify(result,null,2)+'\n');
    console.log(app,replacements,'done');
   }finally{await context.close();}
  }
  fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify({browser:browser.version(),results},null,2)+'\n');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
