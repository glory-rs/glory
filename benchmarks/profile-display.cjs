// Diagnostic CPU/trace samples, kept separate from uninstrumented timing runs.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const root=path.resolve(process.env.GLORY_BENCH_BUILD_ROOT||'D:/BuildCache/glory-display-completion-20261028');
const out=path.join(root,process.argv[2]||'profiles');fs.mkdirSync(out);
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const routes={before:'before/site/glory',api:'first-api/site/glory',after:'after/site/glory',dioxus:'before/site/dioxus'};
routes.glory=process.env.GLORY_CANDIDATE_SITE||'completed/site/glory';
const variants=(process.argv[3]||'before,api,dioxus').split(',');
const server=http.createServer((req,res)=>{try{let file=path.resolve(root,'.'+new URL(req.url,'http://localhost').pathname);assert(file.startsWith(root+path.sep));if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');fs.createReadStream(file).pipe(res);}catch{res.writeHead(404);res.end()}});
(async()=>{
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const evidence={diagnostic:true,variants:[],browser:null};
 try{for(const variant of variants){
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});
  try{evidence.browser=browser.version();const page=await browser.newPage({viewport:{width:1280,height:900}});
   const cdp=await page.context().newCDPSession(page);await cdp.send('Profiler.enable');await cdp.send('Profiler.setSamplingInterval',{interval:100});
   for(const [operation,selector,setup] of [['create10k','.bench-runlots',false],['replace1k','.bench-run',true],['clear1k','.bench-clear',true],['select1k','.bench-row:nth-child(2) .lbl',true]]){
    await page.goto(`http://127.0.0.1:${server.address().port}/${routes[variant]}/`);await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
    if(setup)await page.evaluate(async()=>{document.querySelector('.bench-run').click();await window.__BENCH__.settle()});
    await cdp.send('HeapProfiler.collectGarbage');await cdp.send('Tracing.start',{categories:'devtools.timeline,v8',transferMode:'ReturnAsStream'});await cdp.send('Profiler.start');
    await page.evaluate(async selector=>{performance.mark('display-profile-start');document.querySelector(selector).click();await window.__BENCH__.settle();performance.mark('display-profile-end')},selector);
    const {profile}=await cdp.send('Profiler.stop');const done=new Promise(r=>cdp.once('Tracing.tracingComplete',r));await cdp.send('Tracing.end');const {stream}=await done;
    let trace='';for(;;){const part=await cdp.send('IO.read',{handle:stream});trace+=part.data;if(part.eof)break}await cdp.send('IO.close',{handle:stream});
    fs.writeFileSync(path.join(out,`${variant}-${operation}.cpuprofile`),JSON.stringify(profile));fs.writeFileSync(path.join(out,`${variant}-${operation}.trace.json`),trace);
    const counts=new Map();const nodes=new Map(profile.nodes.map(n=>[n.id,n]));
    for(const id of profile.samples??[]) {const f=nodes.get(id).callFrame;const key=JSON.stringify([f.functionName,f.url,f.lineNumber]);counts.set(key,(counts.get(key)||0)+1)}
    const selfSamples=[...counts].sort((a,b)=>b[1]-a[1]).map(([key,samples])=>({frame:JSON.parse(key),samples}));
    const totals={};for(const event of JSON.parse(trace).traceEvents){if(event.ph==='X'&&event.dur) {const entry=totals[event.name]??={calls:0,inclusive_ms:0};entry.calls++;entry.inclusive_ms+=event.dur/1000}}
    const record={variant,operation,samples:profile.samples?.length,profile_ms:(profile.endTime-profile.startTime)/1000,selfSamples,traceTotals:totals,stats:await page.evaluate(()=>window.__BENCH_STATS__?.()??null)};
    evidence.variants.push(record);console.log(variant,operation,record.samples,selfSamples.slice(0,3));
   }
  }finally{await browser.close()}
 }fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify(evidence,null,2));}finally{server.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
