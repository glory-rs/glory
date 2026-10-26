// Compare two frozen, same-path builds. This measures the Mapper kernel, not DOM/FPS.
'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),cp=require('node:child_process'),assert=require('node:assert/strict'),crypto=require('node:crypto'),os=require('node:os');
const root=path.resolve(process.argv[2]);
assert(root.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const {chromium}=require('../../tests/playwright/node_modules/playwright');
const cases=['plain','heading','navigation','grid','relations','dialog','same_role','named_reset'];
const pairs=2048,samples=20,rounds=4,warmups=5,patches=pairs*2;
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const report={recorded:new Date().toISOString(),scope:'Mapper.apply Role kernel; no DOM or UI callbacks',profile:{optLevel:3,lto:true,codegenUnits:1},optimizer:'Binaryen 132.0.0 -Oz --enable-bulk-memory --enable-reference-types',pairs,samples,rounds,warmups,environment:{node:process.version,os:os.version(),cpu:os.cpus()[0].model},artifacts:{},raw:[],summary:[]};
const output=path.join(root,'performance.json');assert(!fs.existsSync(output),'Keep existing evidence');
fs.copyFileSync(__filename,path.join(root,'performance-driver.cjs'));
function run(exe,args){const r=cp.spawnSync(exe,args,{encoding:'utf8',windowsHide:true,maxBuffer:32*1024*1024});assert.equal(r.status,0,r.stderr||String(r.error));return r.stdout;}
const binaryen='D:/BuildCache/sapid/wasm-size/tools/node_modules/binaryen';
assert.equal(JSON.parse(fs.readFileSync(path.join(binaryen,'package.json'))).version,'132.0.0');
report.environment.rustc=run('rustc',['-Vv']).trim();
for(const variant of ['before','after']){
  const source=path.join(root,variant,'perf-site'),site=path.join(root,variant,'perf-optimized');
  fs.mkdirSync(site,{recursive:false});for(const file of ['app.js','index.html'])fs.copyFileSync(path.join(source,file),path.join(site,file));
  run(process.execPath,[path.join(binaryen,'bin/wasm-opt'),path.join(source,'app_bg.wasm'),'-Oz','--enable-bulk-memory','--enable-reference-types','-o',path.join(site,'app_bg.wasm')]);
  report.artifacts[variant]={native:hash(path.join(root,variant,'target/release/bench-role-patches.exe')),wasm:hash(path.join(site,'app_bg.wasm')),inputs:hash(path.join(root,'inputs-'+variant,'inputs.json'))};
}
function save(){fs.writeFileSync(output,JSON.stringify(report,null,2)+'\n');}
function record(mode,variant,round,entries){
  assert.equal(entries.length,cases.length*samples);
  for(const entry of entries){assert(cases.includes(entry.case));assert.equal(entry.result.patches,patches);report.raw.push({mode,variant,round,...entry});}
  save();console.log(`${mode} ${variant} round ${round+1}/${rounds}`);
}
for(let round=0;round<rounds;round++)for(const variant of round%2?['after','before']:['before','after']){
  const entries=run(path.join(root,variant,'target/release/bench-role-patches.exe'),[String(pairs),String(samples)]).trim().split(/\r?\n/).map(JSON.parse);
  record('native',variant,round,entries);
}
const server=http.createServer((req,res)=>{
  try{
    const url=new URL(req.url,'http://localhost');let file=path.resolve(root,'.'+decodeURIComponent(url.pathname));
    assert(file.startsWith(root+path.sep));if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');
    res.setHeader('Cache-Control','no-store');res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':'text/html');fs.createReadStream(file).pipe(res);
  }catch{res.statusCode=404;res.end('Not found');}
});
(async()=>{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));let browser;
  try{
    browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});report.environment.browser=browser.version();
    for(let round=0;round<rounds;round++)for(const variant of round%2?['after','before']:['before','after']){
      const context=await browser.newContext();try{
        const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
        await page.goto(`http://127.0.0.1:${server.address().port}/${variant}/perf-optimized/`);await page.waitForFunction(()=>window.ready);
        const entries=await page.evaluate(({cases,pairs,samples,warmups})=>{
          const entries=[];
          for(let index=0;index<cases.length;index++){
            const app=window.rolebench;app.setup(index);for(let warm=0;warm<warmups;warm++)app.run(pairs);
            for(let sample=0;sample<samples;sample++){
              const start=performance.now();app.run(pairs);const ns=(performance.now()-start)*1e6;
              entries.push({case:cases[index],sample,ns,result:JSON.parse(app.result())});
            }
            app.clear();
          }return entries;
        },{cases,pairs,samples,warmups});assert.deepEqual(errors,[]);record('wasm',variant,round,entries);
      }finally{await context.close();}
    }
    const quantile=(sorted,q)=>sorted[Math.floor((sorted.length-1)*q)];
    for(const name of cases){
      const results=report.raw.filter(row=>row.case===name).map(row=>JSON.stringify(row.result));
      assert.equal(new Set(results).size,1,`Edit count/order/content differs: ${name}`);
      for(const mode of ['native','wasm']){
        const row={case:name,mode,result:JSON.parse(results[0])};
        for(const variant of ['before','after']){
          const values=report.raw.filter(r=>r.case===name&&r.mode===mode&&r.variant===variant).map(r=>r.ns/patches).sort((a,b)=>a-b);
          assert.equal(values.length,samples*rounds);
          row[variant]={medianNs:quantile(values,.5),q1Ns:quantile(values,.25),q3Ns:quantile(values,.75)};
        }
        row.changePercent=(row.after.medianNs/row.before.medianNs-1)*100;report.summary.push(row);
      }
    }
    report.correctness={identicalNativeAndWasmEditCountsOrderContent:true,noBrowserErrors:true};save();console.log(JSON.stringify(report.summary,null,2));
  }finally{if(browser)await browser.close();server.close();}
})().catch(error=>{report.failure=String(error.stack);save();console.error(error);process.exitCode=1;});
