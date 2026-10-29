'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const root=process.env.GLORY_MIXED_SITE||'D:/BuildCache/glory-semantic-ui/mixed-site';
const snapshot=fs.readFileSync(path.join(root,'snapshot.html'),'utf8').trim();
const errors=[],evidence={checks:[]};
const server=http.createServer((req,res)=>{
    if(req.url==='/favicon.ico'){res.statusCode=204;res.end();return;}
    if(req.url.startsWith('/app')||req.url.startsWith('/snippets/')){const file=path.resolve(root,'.'+req.url);assert(file.startsWith(path.resolve(root)+path.sep));res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':'text/javascript');fs.createReadStream(file).pipe(res);return;}
    const hydrate=req.url.includes('hydrate');
    res.setHeader('Content-Type','text/html; charset=utf-8');
    res.end(`<!doctype html><meta charset="utf-8"><div id="mixed">${hydrate?snapshot:''}</div><script type="module">import init,* as app from '/app.js';await init();window.app=app;window.__READY__=true;</script>`);
});
(async()=>{
    await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
    const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
    try{
        for(const mode of ['csr','hydrate','hydrate-tamper']){
            const page=await browser.newPage();page.on('pageerror',e=>errors.push(e.stack));
            await page.goto(`http://127.0.0.1:${server.address().port}/${mode}`);await page.waitForFunction(()=>window.__READY__);
            if(mode.includes('hydrate')){
                await page.evaluate(()=>{window.__raw=document.querySelector('.raw');window.__editor=document.querySelector('input.editor');window.__editor.value='before hydration';window.__editor.focus();window.__editor.setSelectionRange(2,7);});
                if(mode.endsWith('tamper')){
                    const refused=await page.evaluate(()=>{document.querySelector('.nested span').textContent='tampered';const root=document.getElementById('mixed');const before=root.innerHTML;try{window.app.mixed_mount(true);return false;}catch(e){return root.innerHTML===before&&!root.hasAttribute('data-gly-app')&&document.querySelector('input.editor')===window.__editor&&window.__editor.value==='before hydration'&&window.__editor.selectionStart===2;}});
                    assert(refused);evidence.checks.push('hydration mismatch leaves DOM, dirty input, identity and selection untouched');await page.close();continue;
                }
            }
            await page.evaluate(hydrate=>window.app.mixed_mount(hydrate),mode==='hydrate');
            const settle=()=>page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);});
            const stats=()=>page.evaluate(()=>JSON.parse(window.app.mixed_stats()));
            if(mode==='hydrate')assert(await page.evaluate(()=>window.__raw===document.querySelector('.raw')&&window.__editor===document.querySelector('input.editor')&&window.__editor.value==='before hydration'&&window.__editor.selectionStart===2&&window.__editor.selectionEnd===7));
            for(const selector of ['.raw','.semantic','.opaque','.configured']){await page.locator('#mixed '+selector).click();await settle();}
            assert.equal((await stats()).count,4);assert.equal((await stats()).bubbled,1);assert.equal(await page.locator('.count').textContent(),'4');assert.equal(await page.locator('.count').getAttribute('title'),'count 4');assert.equal(await page.locator('.adjacent').textContent(),'4tail');assert.equal(await page.locator('td > .table-widget').count(),1);assert.equal(await page.locator('.semantic').textContent(),'widget 4');
            await page.locator('input.editor').fill('edited 🦀');await settle();assert.equal(await page.locator('input.editor').inputValue(),'edited 🦀');
            await page.evaluate(()=>{window.__vector=document.querySelector('.vector-1');window.__vector.value='vector draft';window.__vector.setSelectionRange(1,5);window.__row=document.querySelector('.row-button');window.__row.focus();window.__owned=document.querySelector('.owned');window.app.mixed_update();});await settle();
            assert.deepEqual(await page.locator('.row-button').allTextContents(),['row 2','row 1']);
            assert.deepEqual(await page.locator('.vector-label').allTextContents(),['two!','one']);
            assert.deepEqual(await page.locator('.vector-index').allTextContents(),['0','1']);
            assert(await page.evaluate(()=>__vector===document.querySelector('.vector-1')&&__vector.value==='vector draft'&&__vector.selectionStart===1&&__vector.selectionEnd===5));
            assert(await page.evaluate(()=>window.__row===document.querySelectorAll('.row-button')[1]&&document.activeElement===window.__row));
            assert.equal(await page.locator('.mixed.active').count(),0);assert.equal(await page.locator('.branch').count(),0);assert.equal((await stats()).drops,1);
            const hidden=await stats();await page.evaluate(()=>window.__owned.click());await settle();assert.equal((await stats()).count,4);
            for(let n=0;n<5;n++){await page.evaluate(()=>window.app.mixed_update());await settle();await page.evaluate(()=>window.app.mixed_update());await settle();const now=await stats();assert.equal(now.nodes,hidden.nodes);assert.equal(now.owners,hidden.owners);}
            const closed=await page.evaluate(()=>JSON.parse(window.app.mixed_close()));assert.equal(closed.nodes,0);assert.equal(closed.owners,1);assert.equal(await page.locator('#mixed > *').count(),0);
            evidence.checks.push(`${mode}: raw/Widget/factory events, reactive fields/classes, controlled editing, keyed identity/focus, branch release, bounded ownership and teardown`);
            await page.close();
        }
        assert.deepEqual(errors,[]);evidence.passed=true;evidence.browser=browser.version();console.log(JSON.stringify(evidence));
    }finally{fs.writeFileSync(path.join(root,'evidence.json'),JSON.stringify({...evidence,errors},null,2));await browser.close();server.close();}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
