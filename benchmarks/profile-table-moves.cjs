// Isolated diagnostic: same standard bundle, only native move API availability varies.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const root=process.env.GLORY_BENCH_BUILD_ROOT||'D:/BuildCache/glory-display-completion-20261028';
const out=path.join(root,process.argv[2]||'table-move-diagnostic');fs.mkdirSync(out);
const selectionExperiment=process.argv[3]==='selection';
const cacheExperiment=process.argv[3]==='selection-cache';
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const median=values=>{values=[...values].sort((a,b)=>a-b);return(values[values.length/2-1]+values[values.length/2])/2};
(async()=>{const records=[];
 for(let round=0;round<2;round++)for(const fallback of round?[true,false]:[false,true]){
  const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
  try{for(let sample=0;sample<12;sample++){
   const page=await browser.newPage({viewport:{width:1280,height:900}});
   if(fallback&&cacheExperiment)await page.route('**/template_host.js',route=>route.fulfill({contentType:'text/javascript',body:fs.readFileSync(path.join(root,'selection-cache/inputs/crates/web/src/template_host.js'),'utf8')}));
   await page.addInitScript(({fallback,selectionExperiment,cacheExperiment})=>{
    window.__moves={atomic:0,insert:0};
    const atomic=Element.prototype.moveBefore,insert=Node.prototype.insertBefore;
    if(fallback&&selectionExperiment)Document.prototype.getSelection=function(){return null};
    if(fallback&&!selectionExperiment&&!cacheExperiment)Object.defineProperty(Element.prototype,'moveBefore',{value:undefined,configurable:true});
    else Element.prototype.moveBefore=function(...args){window.__moves.atomic++;return atomic.apply(this,args)};
    Node.prototype.insertBefore=function(...args){window.__moves.insert++;return insert.apply(this,args)};
    window.__settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
   },{fallback,selectionExperiment,cacheExperiment});
   await page.goto('http://localhost:8080/frameworks/keyed/glory/bundled-dist/index.html');
   await page.waitForSelector('#run');await page.evaluate(async()=>{document.querySelector('#run').click();await window.__settle();for(let i=0;i<5;i++){document.querySelector('#swaprows').click();await window.__settle()}});
   const cdp=await page.context().newCDPSession(page);await cdp.send('Performance.enable');await cdp.send('Emulation.setCPUThrottlingRate',{rate:4});
   const metrics=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(v=>[v.name,v.value]));
   const before=await metrics();
   const result=await page.evaluate(async()=>{const rows=document.querySelectorAll('tbody>tr'),row=rows[1],text=row.querySelector('td').textContent;window.__moves={atomic:0,insert:0};const start=performance.now();document.querySelector('#swaprows').click();await window.__settle();return{wall_ms:performance.now()-start,moves:window.__moves,identity:document.querySelectorAll('tbody>tr')[998]===row,text:document.querySelectorAll('tbody>tr')[998].querySelector('td').textContent===text}});
   const after=await metrics();assert(result.identity&&result.text);assert.equal(result.moves.atomic+result.moves.insert,2);
   records.push({round,fallback,sample,...result,metrics:Object.fromEntries(['ScriptDuration','LayoutDuration','RecalcStyleDuration','TaskDuration'].map(k=>[k,(after[k]-before[k])*1000]))});
   await page.close();
  }}finally{await browser.close()}
 }
 const summary=[false,true].map(fallback=>({fallback,ScriptDuration:median(records.filter(r=>r.fallback===fallback).map(r=>r.metrics.ScriptDuration)),wall_ms:median(records.filter(r=>r.fallback===fallback).map(r=>r.wall_ms))}));
 fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify({diagnostic:true,factor:cacheExperiment?'cached-selection-read':selectionExperiment?'omit-selection-read':'native-move-api',rate:4,samples_per_mode:24,records,summary},null,2));console.log(summary);
})().catch(e=>{console.error(e);process.exitCode=1});
