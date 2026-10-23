const fs=require('node:fs'),path=require('node:path');
if(!/^[dD]:[\\/]BuildCache[\\/]/.test(process.env.TEMP||''))throw Error('TEMP must be on D:/BuildCache');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const url=process.env.GLORY_STREAM_URL||'http://127.0.0.1:8789';
(async()=>{const browser=await chromium.launch({channel:'msedge',headless:true});try{
 const page=await browser.newPage(),errors=[];page.on('pageerror',error=>errors.push(error.message));
 page.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
 await page.goto(url+'/page',{waitUntil:'commit'});await page.waitForSelector('.stream-rich');
 await page.locator('.stream-rich').click();await page.keyboard.press('Control+a');await page.keyboard.type('draft before hydration');
 await page.waitForFunction(()=>window.ready&&document.getElementById('stream-root').dataset.glyStreamState==='ready');
 let value=await page.locator('.stream-rich').innerText();if(value.trim()!=='draft before hydration')throw Error('stream overwrote user rich draft: '+value);
 await page.evaluate(()=>{window.richOriginal=document.querySelector('.stream-rich');window.demo.stream_hydrate();});
 await page.waitForFunction(()=>window.demo.stream_rich_text()==='draft before hydration');
 if(!await page.evaluate(()=>window.richOriginal===document.querySelector('.stream-rich')))throw Error('hydrate replaced rich root');
 await page.keyboard.press('End');await page.keyboard.type(' + live');
 await page.waitForFunction(()=>window.demo.stream_rich_text()==='draft before hydration + live');
 const cdp=await page.context().newCDPSession(page);await cdp.send('Input.imeSetComposition',{text:'你好',selectionStart:2,selectionEnd:2});await cdp.send('Input.insertText',{text:'你好'});
 await page.waitForFunction(()=>window.demo.stream_rich_text()==='draft before hydration + live你好');
 await page.keyboard.press('Enter');await page.keyboard.type('next paragraph');
 await page.waitForFunction(()=>window.demo.stream_rich_text().includes('next paragraph'));
 const result=await page.evaluate(()=>({text:window.demo.stream_rich_text(),paragraphs:document.querySelectorAll('.stream-rich [data-gly-rich-block]').length,marker:document.getElementById('stream-root').firstChild.nodeValue}));
 if(result.marker!=='gly:v:4'||result.paragraphs<2)throw Error(JSON.stringify(result));
 await page.evaluate(()=>window.demo.unmount());if(await page.locator('[data-gly-node]').count())throw Error('rich owner teardown');
 if(errors.length)throw Error(errors.join('\n'));console.log(JSON.stringify({streamRich:result,errors}));
 // Preserve the full existing non-rich stream/hydration acceptance too.
 const original=fs.readFileSync(path.join(__dirname,'../examples/semantic-web/stream-acceptance.js'),'utf8').replaceAll('http://127.0.0.1:8789',url);
 console.log(JSON.stringify({existingStream:await eval('('+original+')')(page)}));
}finally{await browser.close();}})().catch(error=>{console.error(error);process.exitCode=1;});
