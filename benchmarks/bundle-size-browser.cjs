// Verify the same row contract before and after Binaryen; no timing benchmark.
'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),vm=require('node:vm'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const root=path.resolve(process.argv[2]||'D:/BuildCache/glory-benchmark-wasm-20261026');
if(!root.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'))throw Error('Evidence and browser output must be on D:');
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const {chromium}=require('../tests/playwright/node_modules/playwright');
const out=path.join(root,'bundle-size'),site=path.join(out,'site');
const report=JSON.parse(fs.readFileSync(path.join(out,'report.json')));
const acceptanceSource=fs.readFileSync(path.join(__dirname,'acceptance.js'),'utf8');
const acceptance=vm.runInThisContext('('+acceptanceSource+')',{filename:'acceptance.js'});
const evidenceRoot=path.join(out,'browser');fs.mkdirSync(evidenceRoot,{recursive:true});
fs.writeFileSync(path.join(evidenceRoot,'acceptance.js'),acceptanceSource);
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const server=http.createServer((req,res)=>{
  try{
    const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
    let file=path.resolve(site,'.'+pathname);if(!file.startsWith(site+path.sep))throw Error('Invalid path');
    if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');
    res.setHeader('Cache-Control','no-store');
    res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript; charset=utf-8':file.endsWith('.css')?'text/css; charset=utf-8':'text/html; charset=utf-8');
    fs.createReadStream(file).pipe(res);
  }catch{res.statusCode=404;res.end('Not found');}
});
(async()=>{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  let browser;
  const evidence={contract:report.contract,recorded:new Date().toISOString(),cases:[]};
  try{
    browser=await chromium.launch({headless:true,executablePath:process.env.BROWSER_EXECUTABLE||'C:/Program Files/Google/Chrome/Application/chrome.exe'});
    evidence.browser=browser.version();let states,geometry,picture;
    for(const stage of ['bindgen','optimized'])for(const app of report.cases){
      const wasm=path.join(site,stage,app.name,'app_bg.wasm');assert.equal(hash(fs.readFileSync(wasm)),app[stage].sha256);
      const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1,locale:'en-US',colorScheme:'light'});
      try{
        const page=await context.newPage();page.setDefaultTimeout(60000);
        const errors=[];page.on('pageerror',error=>errors.push(error.message));
        const dir=path.join(evidenceRoot,stage);fs.mkdirSync(dir,{recursive:true});page.__BENCH_ARTIFACT_DIR=dir;
        await page.goto(`http://127.0.0.1:${server.address().port}/${stage}/${app.name}/`);await page.bringToFront();
        const result=await acceptance(page);assert.equal(errors.length,0);
        const actual=result.snapshots.map(({stage,count,sha256})=>({stage,count,sha256}));
        if(states)assert.deepEqual(actual,states);else states=actual;
        if(geometry)assert.deepEqual(result.geometry,geometry);else geometry=result.geometry;
        const png=hash(fs.readFileSync(path.join(dir,app.name+'-conformance.png')));
        if(picture)assert.equal(png,picture,'Viewport pixels differ');else picture=png;
        fs.writeFileSync(path.join(dir,app.name+'.json'),JSON.stringify(result,null,2)+'\n');
        evidence.cases.push({name:app.name,stage,sha256:app[stage].sha256,passed:true,seededData:true,keyedIdentity:true,all10000RowsMounted:result.mounted,cleanup:true,pixelsEqual:true});
        fs.writeFileSync(path.join(evidenceRoot,'evidence.json'),JSON.stringify(evidence,null,2)+'\n');
        console.log(`PASS ${app.name} ${stage}: rows, edits, selection, keyed swap, cleanup, pixels`);
      }finally{await context.close();}
    }
  }finally{if(browser)await browser.close();server.close();}
})().catch(error=>{fs.writeFileSync(path.join(evidenceRoot,'failure.txt'),String(error.stack));console.error(error);process.exitCode=1;});
