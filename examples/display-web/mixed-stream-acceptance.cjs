'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict'),{spawn}=require('node:child_process');
const {chromium}=require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const site=process.env.GLORY_MIXED_STREAM_SITE||'D:/BuildCache/glory-semantic-ui/mixed-stream-site';
const binary=process.env.GLORY_MIXED_STREAM_BIN||'D:/BuildCache/glory-semantic-ui/target/debug/mixed-stream-ssr.exe';
process.env.TEMP=process.env.TMP='D:/BuildCache/glory-semantic-ui/tmp';fs.mkdirSync(process.env.TEMP,{recursive:true});
const asset=path.resolve(__dirname,'../../crates/web/src/stream.js');
const requests=new Map(),errors=[],evidence={checks:[]};
const server=http.createServer((req,res)=>{
    const url=new URL(req.url,'http://localhost');
    if(url.pathname==='/page'){
        const id=url.searchParams.get('id');const child=spawn(binary,[],{windowsHide:true,env:{...process.env,TEMP:'D:/BuildCache/glory-semantic-ui/tmp',TMP:'D:/BuildCache/glory-semantic-ui/tmp'}});
        const record={child,bytes:[],stderr:'',done:false};requests.set(id,record);
        res.setHeader('Content-Type','text/html; charset=utf-8');res.setHeader('Content-Security-Policy',"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; connect-src 'self'");
        child.stdout.on('data',data=>record.bytes.push(data));child.stdout.pipe(res);child.stderr.on('data',data=>record.stderr+=data);
        child.on('close',code=>{record.done=true;record.code=code;if(code!==0)errors.push(record.stderr);});
        res.on('close',()=>{if(!res.writableFinished)child.stdin.end();});return;
    }
    if(url.pathname==='/step'){const record=requests.get(url.searchParams.get('id'));record.child.stdin.write(url.searchParams.get('stage')+'\n');res.statusCode=204;res.end();return;}
    if(url.pathname==='/loader.js'){res.setHeader('Content-Type','text/javascript');res.end("import init,* as app from '/app.js';await init();window.app=app;window.__READY__=true;");return;}
    if(url.pathname==='/empty'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><script src="/stream.js"></script>');return;}
    if(url.pathname==='/stream.js'){res.setHeader('Content-Type','text/javascript');fs.createReadStream(asset).pipe(res);return;}
    if(url.pathname==='/favicon.ico'){res.statusCode=204;res.end();return;}
    const file=path.resolve(site,'.'+url.pathname);if(!file.startsWith(path.resolve(site)+path.sep)||!fs.existsSync(file)){res.statusCode=404;res.end();return;}
    res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':'text/javascript');fs.createReadStream(file).pipe(res);
});
const until=async test=>{for(let attempt=0;attempt<100;attempt++){if(test())return;await new Promise(resolve=>setTimeout(resolve,30));}assert.fail('server cleanup timeout');};
(async()=>{
    await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base=`http://127.0.0.1:${server.address().port}`;
    const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe'});
    try{
        const page=await browser.newPage();page.on('pageerror',error=>errors.push(error.stack));
        const response=await page.goto(base+'/page?id=main',{waitUntil:'commit'});
        await page.waitForFunction(()=>window.GloryStream?.active()===1&&document.querySelector('.value')?.textContent==='0tail');
        const shell=await page.locator('#stream-root').innerHTML();
        await page.evaluate(()=>{Element.prototype.moveBefore=undefined;DocumentFragment.prototype.moveBefore=undefined;
            const editor=document.querySelector('.editor'),row=document.querySelector('.row-1');window.saved={editor,row,raw:document.querySelector('.raw'),vector:document.querySelector('.vector-1')};saved.vector.value='vector draft';saved.vector.setSelectionRange(1,5);
            editor.value='typed while streaming 🦀';editor.dispatchEvent(new InputEvent('input',{bubbles:true}));row.value='row draft';row.dispatchEvent(new InputEvent('input',{bubbles:true}));row.focus();row.setSelectionRange(1,5);
            window.valueWrites=0;const value=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value');Object.defineProperty(HTMLInputElement.prototype,'value',{...value,set(text){if(this===editor||this===row)window.valueWrites++;value.set.call(this,text);}});
        });
        assert.equal(await page.locator('.suspended').count(),0);assert.equal(await page.locator('tbody tr').count(),3);
        const step=async stage=>{await fetch(`${base}/step?id=main&stage=${stage}`);await page.waitForFunction(stage=>document.querySelector('.value')?.textContent===`${stage}tail`,stage);};
        await step(1);assert.equal(await page.locator('.branch').count(),1);assert.equal(await page.locator('.stream-mixed.active').count(),1);assert.equal(await page.locator('.suspended').count(),1);
        await page.evaluate(()=>{window.saved.suspended=document.querySelector('.suspended-editor');window.saved.retired=document.querySelector('.row-3');window.saved.weakBranch=new WeakRef(document.querySelector('.branch'));});
        await step(2);assert.equal(await page.locator('.branch').count(),0);assert.equal(await page.locator('.stream-mixed.active').count(),0);assert.equal(await page.locator('.suspended').count(),0);
        assert.equal(await page.evaluate(()=>saved.suspended.parentElement.querySelector('span').textContent),'2');
        assert(await page.evaluate(()=>saved.row===document.querySelector('.row-1')&&document.activeElement===saved.row&&saved.row.selectionStart===1&&saved.row.selectionEnd===5));
        await step(3);await page.waitForFunction(()=>window.__READY__&&document.getElementById('stream-root').dataset.glyStreamState==='ready');
        assert.equal(await page.locator('.stream-mixed.active').count(),1);
        const ended=await page.evaluate(()=>({same: saved.editor===document.querySelector('.editor')&&saved.row===document.querySelector('.row-1')&&saved.raw===document.querySelector('.raw')&&saved.suspended===document.querySelector('.suspended-editor'),editor:saved.editor.value,row:saved.row.value,focus:document.activeElement===saved.row,selection:[saved.row.selectionStart,saved.row.selectionEnd],writes:valueWrites,active:GloryStream.active(),first:document.getElementById('stream-root').firstChild.nodeValue,order:[...document.querySelectorAll('.rows input')].map(node=>node.className.includes('row-2')?2:1),named:document.querySelector('.named-value').textContent,title:document.querySelector('.value').title,rows:document.querySelectorAll('tbody tr').length}));
        assert(ended.same&&ended.focus);assert.equal(ended.editor,'typed while streaming 🦀');assert.equal(ended.row,'row draft');assert.deepEqual(ended.selection,[1,5]);assert.equal(ended.writes,0);assert.equal(ended.active,0);assert.equal(ended.first,'gly:v:14');assert.deepEqual(ended.order,[2,1]);assert.equal(ended.named,'3');assert.equal(ended.title,'value 3');assert.equal(ended.rows,2);
        assert.deepEqual(await page.locator('.vector-label').allTextContents(),['two 3','one 3']);
        assert.deepEqual(await page.locator('.vector-index').allTextContents(),['0','1']);
        assert(await page.evaluate(()=>saved.vector===document.querySelector('.vector-1')&&saved.vector.value==='vector draft'&&saved.vector.selectionStart===1&&saved.vector.selectionEnd===5));
        evidence.checks.push('real gated HTTP stream: raw fields/attributes/classes, named Children, nested Widgets, Show disposal/recreation, table Each, initial suspension/resume and fallback keyed movement preserve editors/focus/selection');
        await page.evaluate(()=>window.app.mixed_stream_hydrate());
        assert(await page.evaluate(()=>saved.row===document.querySelector('.row-1')&&saved.editor===document.querySelector('.editor')&&saved.editor.value==='typed while streaming 🦀'&&document.activeElement===saved.row&&valueWrites===0&&!document.getElementById('stream-data')));
        for(const selector of ['.raw','.factory','.branch-raw'])await page.locator(selector).click();
        await page.waitForFunction(()=>JSON.parse(window.app.mixed_stream_stats()).count===6);assert.equal(await page.locator('.value').textContent(),'6tail');
        await page.evaluate(()=>{window.stale=document.querySelector('.branch-raw');app.mixed_stream_toggle();});await page.waitForFunction(()=>!document.querySelector('.branch'));
        await page.evaluate(()=>window.stale.click());assert.equal(await page.evaluate(()=>JSON.parse(app.mixed_stream_stats()).count),6);assert.equal(await page.evaluate(()=>JSON.parse(app.mixed_stream_stats()).drops),1);
        await page.evaluate(()=>app.mixed_stream_close());assert.equal(await page.locator('#stream-root > *').count(),0);
        evidence.checks.push('End rekeys outlet markers and hands the same DOM to DSL hydration; raw/factory callbacks work, retired raw callbacks and captures release');
        const raw=await response.text();fs.writeFileSync(path.join(site,'response.html'),raw);const frames=[...raw.matchAll(/<script type="application\/json" data-gly-stream="stream-root"[^>]*>([\s\S]*?)<\/script>/g)].map(match=>JSON.parse(match[1]));
        const definitions=frames.flatMap(frame=>frame.nodes||frame.ops?.filter(op=>op.op==='create')||[]).filter(node=>node.shape?.display?.wire).length;
        assert(definitions>0);assert(frames.some(frame=>frame.ops?.some(op=>op.op==='display')));assert(frames.some(frame=>frame.ops?.some(op=>op.op==='suspend')));
        evidence.frames=frames.length;evidence.declarations=definitions;
        const cdp=await page.context().newCDPSession(page);for(let i=0;i<4;i++)await cdp.send('HeapProfiler.collectGarbage');await cdp.detach();
        assert(await page.evaluate(()=>!saved.weakBranch.deref()));evidence.checks.push('retired raw branch DOM is garbage collected');
        // A mixed template can finish streaming while its Widget editor is composing.
        await page.goto(base+'/page?id=composition',{waitUntil:'commit'});await page.waitForFunction(()=>GloryStream.active()===1);
        await page.evaluate(()=>{const input=document.querySelector('.editor');window.composing=input;input.focus();input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));input.value='composition draft';input.setSelectionRange(2,7);input.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true}));});
        for(let stage=1;stage<=3;stage++){await fetch(`${base}/step?id=composition&stage=${stage}`);await page.waitForFunction(stage=>document.querySelector('.value').textContent===`${stage}tail`,stage);}
        await page.waitForFunction(()=>window.__READY__&&document.readyState==='complete');
        assert(await page.evaluate(()=>{try{app.mixed_stream_hydrate();return false;}catch(_){return GloryStream.active()===1&&document.getElementById('stream-root').dataset.glyStreamState==='active'&&composing.value==='composition draft';}}));
        await page.evaluate(()=>composing.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})));await page.waitForFunction(()=>document.getElementById('stream-root').dataset.glyStreamState==='ready');
        await page.evaluate(()=>app.mixed_stream_hydrate());assert(await page.evaluate(()=>composing===document.querySelector('.editor')&&composing.value==='composition draft'&&composing.selectionStart===2&&composing.selectionEnd===7&&document.activeElement===composing));await page.evaluate(()=>app.mixed_stream_close());
        evidence.checks.push('mixed-tree End waits for composition, then preserves the editor and selection through hydration');
        // Protocol validation runs against the genuine initial shell/manifest.
        await page.goto(base+'/empty');await page.waitForFunction(()=>window.GloryStream);
        const rejected=await page.evaluate(({shell,initial})=>{
            const run=(change,patch,claimed=false)=>{const root=document.createElement('div');root.id='validation-root';root.setAttribute('data-gly-stream-pending','');if(claimed)root.setAttribute('data-gly-app','');root.innerHTML=shell;document.body.append(root);const data=structuredClone(initial);data.data_id='validation-data';change(data);const before=root.innerHTML;let failed=false;try{const controller=GloryStream.start(root,data);if(patch)controller.apply(patch);else controller.cancel();}catch(_){failed=true;}const unchanged=root.innerHTML===before;GloryStream.cancel(root);root.remove();return failed&&unchanged&&GloryStream.active()===0;};
            const rawRoot=initial.nodes.find(node=>node.shape.display?.kind==='root');
            return [
                run(data=>{const display=data.nodes.find(node=>node.key===rawRoot.key).shape.display;const wire=JSON.parse(display.wire);wire[0][0][1]='script';display.wire=JSON.stringify(wire);}),
                run(()=>{},{version:2,kind:'patch',sequence:1,ops:[{op:'display',key:rawRoot.key,values:[[999,'bad']]}]}),
                run(data=>{data.nodes.find(node=>node.key===rawRoot.key).shape.display.values[0]={bad:true};}),
                run(()=>{},undefined,true),
            ];
        },{shell,initial:frames[0]});assert.deepEqual(rejected,[true,true,true,true]);
        evidence.checks.push('malformed declaration, out-of-range slot and invalid field payload are refused without rewriting the existing shell');
        await page.goto(base+'/page?id=cancel',{waitUntil:'commit'});await page.waitForFunction(()=>GloryStream.active()===1);await page.goto(base+'/empty');await until(()=>requests.get('cancel').done);const cancelled=JSON.parse(requests.get('cancel').stderr.trim());assert.equal(cancelled.completed,false);assert.equal(cancelled.nodes,0);assert.equal(cancelled.tasks,0);
        evidence.checks.push('disconnect cancels a pending server task and releases template instances');
        await until(()=>requests.get('main').done);assert.equal(requests.get('main').code,0);const complete=JSON.parse(requests.get('main').stderr.trim());assert(complete.completed&&complete.nodes===0&&complete.tasks===0&&complete.drops===2);assert.deepEqual(errors,[]);evidence.passed=true;evidence.browser=browser.version();console.log(JSON.stringify(evidence));
    }finally{fs.writeFileSync(path.join(site,'evidence.json'),JSON.stringify({...evidence,errors},null,2));await browser.close();for(const record of requests.values())if(!record.done)record.child.stdin.end();server.close();}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
