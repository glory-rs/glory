// Exact JS bridge counts through V8 coverage, outside all timing runs.
const fs=require('node:fs'),http=require('node:http'),path=require('node:path'),assert=require('node:assert/strict');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const root=path.resolve(process.env.GLORY_BENCH_BUILD_ROOT||'D:/BuildCache/glory-display-completion-20261028');
const routes={before:'before/site/glory',glory:process.env.GLORY_CANDIDATE_SITE||'completed/site/glory'};
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const output=path.join(root,'submit-counts');fs.mkdirSync(output);
const server=http.createServer((req,res)=>{try{let file=path.resolve(root,'.'+new URL(req.url,'http://localhost').pathname);assert(file.startsWith(root+path.sep));if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');fs.createReadStream(file).pipe(res)}catch{res.writeHead(404);res.end()}});
(async()=>{await new Promise(r=>server.listen(0,'127.0.0.1',r));const records=[];
 try{for(const [variant,route] of Object.entries(routes)){
  const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
  try{const page=await browser.newPage();const cdp=await page.context().newCDPSession(page);await cdp.send('Profiler.enable');
   for(const [operation,selector,setup] of [['create10k','.bench-runlots',false],['update1k','.bench-update',true],['clear1k','.bench-clear',true]]){
    await page.goto(`http://127.0.0.1:${server.address().port}/${route}/`);await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
    if(setup)await page.evaluate(async()=>{document.querySelector('.bench-run').click();await window.__BENCH__.settle()});
    await cdp.send('Profiler.startPreciseCoverage',{callCount:true,detailed:false});
    await page.evaluate(async s=>{document.querySelector(s).click();await window.__BENCH__.settle()},selector);
    const coverage=await cdp.send('Profiler.takePreciseCoverage');await cdp.send('Profiler.stopPreciseCoverage');
    const script=coverage.result.find(s=>s.url.endsWith('/template_host.js'));assert(script);
    const counts=Object.fromEntries(['submit','defineTemplate'].map(name=>[name,script.functions.find(f=>f.functionName===name)?.ranges[0].count??0]));
    assert(counts.submit>0);records.push({variant,operation,...counts,stats:await page.evaluate(()=>window.__BENCH_STATS__())});
   }
  }finally{await browser.close()}
 }fs.writeFileSync(path.join(output,'evidence.json'),JSON.stringify({diagnostic:true,method:'V8 precise JS coverage; never used as timing evidence',records},null,2));console.log(records.map(({variant,operation,submit})=>({variant,operation,submit})));}finally{server.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
