'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const root=path.resolve(process.env.GLORY_DSL_BUILD_ROOT||'D:/BuildCache/glory-dsl-20261028');
process.env.TEMP=process.env.TMP=path.join(root,'tmp');
const output=path.join(root,process.argv[2]||'acceptance');fs.mkdirSync(output);
const server=http.createServer((req,res)=>{
 try {let file=path.resolve(root,'.'+new URL(req.url,'http://localhost').pathname);assert(file.startsWith(root+path.sep));if(fs.statSync(file).isDirectory())file=path.join(file,'index.html');
 res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript':'text/html');res.setHeader('Cache-Control','no-store');fs.createReadStream(file).pipe(res);
 }catch {res.statusCode=404;res.end();}
});
const evidence={runs:[]},save=()=>fs.writeFileSync(path.join(output,'evidence.json'),JSON.stringify(evidence,null,2)+'\n');
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',downloadsPath:path.join(root,'downloads')});
 try {
  evidence.browser=browser.version();let canonical,pixels;
  for(const variant of ['hand','dsl']){
   const context=await browser.newContext({viewport:{width:900,height:700},deviceScaleFactor:1});const page=await context.newPage();
   const errors=[];page.on('pageerror',e=>errors.push(e.message));
   await page.goto(`http://127.0.0.1:${server.address().port}/${variant}/site/`);await page.bringToFront();await page.waitForFunction(()=>window.__READY__);
   const settle=()=>page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);});
   const click=async selector=>{await page.locator(selector).evaluate(e=>e.click());await settle();};
   const state=()=>page.evaluate(()=>({root:document.querySelector('.demo>.value').textContent,computed:document.querySelector('.computed').textContent,once:document.querySelector('.once').textContent,child:document.querySelector('.counter>.value').textContent,double:document.querySelector('.counter>.double').textContent,stats:JSON.parse(window.app.stats())}));
   const snapshots=[];const baseline=await state();snapshots.push(baseline);
   assert.equal(await page.evaluate(()=>window.app.computed_calls()),1);
   assert.deepEqual([baseline.root,baseline.computed,baseline.once,baseline.child,baseline.double],['0','0','0','10','20']);
   await page.evaluate(()=>{window.__elements=[...document.querySelectorAll('#main *')];window.__text=[];const walker=document.createTreeWalker(document.getElementById('main'),NodeFilter.SHOW_TEXT);while(walker.nextNode())window.__text.push(walker.currentNode);});
   await click('.demo>.increment');let current=await state();assert.deepEqual([current.root,current.computed,current.once],['1','1','0']);snapshots.push(current);
   await click('.toggle');current=await state();assert.equal(current.computed,'100');snapshots.push(current);
   assert.equal(await page.evaluate(()=>window.app.computed_calls()),3);
   await click('.demo>.increment');current=await state();assert.deepEqual([current.root,current.computed],['2','100']);snapshots.push(current);
   assert.equal(await page.evaluate(()=>window.app.computed_calls()),3,'branch retained inactive dependency');
   await click('.alt');current=await state();assert.equal(current.computed,'101');snapshots.push(current);
   await click('.counter>.increment');current=await state();assert.deepEqual([current.child,current.double],['11','22']);snapshots.push(current);
   assert.equal(await page.evaluate(()=>window.app.computed_calls()),4);
   await page.evaluate(()=>window.app.set_class('demo changed'));assert.equal(await page.locator('.demo').getAttribute('class'),'demo changed');
   assert(await page.evaluate(()=>{
    const elements=[...document.querySelectorAll('#main *')],texts=[],walker=document.createTreeWalker(document.getElementById('main'),NodeFilter.SHOW_TEXT);while(walker.nextNode())texts.push(walker.currentNode);
    return elements.every((node,i)=>node===window.__elements[i])&&texts.every((node,i)=>node===window.__text[i]);
   }),'field update replaced static nodes');
   assert.equal(await page.locator('.escaped').textContent(),'<div>"汉字🙂&');
   assert.equal(await page.locator('.escaped>*').count(),0,'text injected HTML');
   await page.evaluate(()=>window.app.add_rows(600));await settle();
   const full=await state();assert.equal(await page.locator('.row').count(),600);assert.equal(full.stats.instances,baseline.stats.instances+600);snapshots.push(full);
   await click('.row:first-child');assert.equal((await state()).root,'102');
   await page.evaluate(()=>{window.__stale=document.querySelector('.row');window.__weak=new WeakRef(document.querySelectorAll('.row')[1]);window.__weakText=new WeakRef(document.querySelectorAll('.row')[1].firstChild);window.app.clear_rows();});await settle();
   const retired=await state();assert.deepEqual(retired.stats,baseline.stats);
   await page.evaluate(()=>window.__stale.click());await settle();assert.equal((await state()).root,'102');await page.evaluate(()=>{window.__stale=null;});
   for(let i=0;i<5;i++){await page.evaluate(()=>{window.app.add_rows(80);window.app.clear_rows();});await settle();assert.deepEqual((await state()).stats,baseline.stats);}
   for(let i=0;i<3;i++){await page.evaluate(()=>{window.app.add_components(10);});await settle();assert.equal((await state()).stats.instances,baseline.stats.instances+10);await page.evaluate(()=>window.app.clear_rows());await settle();assert.deepEqual((await state()).stats,baseline.stats,'component derived map escaped its scope');}
   const cdp=await context.newCDPSession(page);for(let i=0;i<4;i++)await cdp.send('HeapProfiler.collectGarbage');await cdp.detach();
   assert(await page.evaluate(()=>!window.__weak.deref()&&!window.__weakText.deref()),'retired rows retained');
   const structural=await require('./structural-acceptance.cjs')(page);
   evidence.structural??=[];evidence.structural.push({variant,...structural});
   const html=await page.locator('#main').innerHTML();
   if(canonical){assert.deepEqual(snapshots,canonical.snapshots);assert.equal(html,canonical.html);}else canonical={snapshots,html};
   const png=await page.screenshot({path:path.join(output,variant+'.png')});const hash=crypto.createHash('sha256').update(png).digest('hex');if(pixels)assert.equal(hash,pixels);else pixels=hash;
   await page.evaluate(()=>window.app.close());assert.equal(await page.locator('#main>*').count(),0);assert.deepEqual(errors,[]);
   evidence.runs.push({variant,baseline,snapshots,html,screenshot_sha256:hash,errors,teardown:true,collected:true});save();await context.close();
   console.log('Accepted '+variant+': shared DOM, conditional dependencies, component state, 600 rows, stale events, GC, teardown');
  }
  if(fs.existsSync(path.join(root,'counter/site'))){
   const context=await browser.newContext({viewport:{width:900,height:700}});const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
   await page.goto(`http://127.0.0.1:${server.address().port}/counter/site/`);await page.waitForFunction(()=>window.__READY__);
   const click=async selector=>{await page.locator(selector).click();await page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame)});};
   await click('.increment');assert.equal(await page.locator('.counter>.value').textContent(),'1');
   for(let i=0;i<3;i++)await click('.add');assert.equal(await page.locator('.total').textContent(),'3');
   await page.evaluate(()=>window.__firstItem=document.querySelector('.item'));
   await click('.item:first-child .edit');assert.equal(await page.locator('.item:first-child .select').textContent(),'Item 1!');
   await click('.item:first-child .select');assert.equal(await page.locator('.item.selected').count(),1);
   await click('.item:nth-child(2) .remove');assert.equal(await page.locator('.total').textContent(),'2');
   assert(await page.evaluate(()=>window.__firstItem===document.querySelector('.item')));
   await click('.toggle');assert.equal(await page.locator('.counter').count(),0);await click('.toggle');assert.equal(await page.locator('.counter>.value').textContent(),'0');
   await click('.clear');assert.equal(await page.locator('.total').textContent(),'0');assert.equal(await page.locator('.item').count(),0);assert.deepEqual(errors,[]);
   await page.screenshot({path:path.join(output,'counter.png')});evidence.counter={passed:true,component:true,each:true,field_edit:true,class_flag:true,branch_reset:true,identity:true,errors};save();await context.close();
   console.log('Accepted ordinary counter/list example: Widget launch, typed setters, children, Each, field edits, class flags and branch reset');
  }
  const rowRoute=process.env.GLORY_ROWS_SITE||'compat-row/site/glory';
  const rowSite=path.join(root,rowRoute);
  if(fs.existsSync(rowSite)){
   const vm=require('node:vm'),repo=path.resolve(__dirname,'../..');
   const oracle=vm.runInThisContext('('+fs.readFileSync(path.join(repo,'benchmarks/acceptance.js'),'utf8')+')');
   const sparse=vm.runInThisContext('('+fs.readFileSync(path.join(repo,'benchmarks/display-acceptance.js'),'utf8')+')');
   const context=await browser.newContext({viewport:{width:1280,height:900},deviceScaleFactor:1});const page=await context.newPage();page.__BENCH_ARTIFACT_DIR=output;
   const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(`http://127.0.0.1:${server.address().port}/${rowRoute}/`);await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);
   evidence.compatibility={oracle:await oracle(page),sparse:await sparse(page)};assert.deepEqual(errors,[]);await context.close();
   console.log('Accepted legacy sparse runtime: seeded 10k oracle, density, focus/selection, memory growth, four fault prefixes and GC');
  }
  evidence.completed=new Date().toISOString();save();
 }catch(e){evidence.failure=String(e.stack);save();throw e;}
 finally {await browser.close();server.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
