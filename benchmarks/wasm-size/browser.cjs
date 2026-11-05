const { localBuildPath } = require("../../scripts/local-build-paths.cjs");
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const root = path.resolve(process.argv[2] || localBuildPath('glory/wasm-size/runs/baseline'));
const buildRoot = path.resolve(root,'../../..');
process.env.TEMP = process.env.TMP = path.join(buildRoot,'tmp');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || path.resolve(__dirname,'../../tests/playwright/node_modules/playwright'));
const report = JSON.parse(fs.readFileSync(path.join(root,'report.json')));
const server = http.createServer((req,res) => {
  try {
    const name = decodeURIComponent(new URL(req.url,'http://localhost').pathname).slice(1);
    const file = path.resolve(root,name);
    if (!file.startsWith(root+path.sep) || !fs.statSync(file).isFile()) throw Error('Not found');
    res.setHeader('Content-Type',file.endsWith('.wasm')?'application/wasm':file.endsWith('.js')?'text/javascript; charset=utf-8':'text/html; charset=utf-8');
    res.setHeader('Cache-Control','no-store');
    fs.createReadStream(file).pipe(res);
  } catch { res.statusCode=404;res.end('Not found'); }
});
function check(value,message) { if(!value) throw Error(message); }
(async () => {
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const browser = await chromium.launch({headless:true, executablePath: process.env.BROWSER_EXECUTABLE || 'C:/Program Files/Google/Chrome/Application/chrome.exe'});
  const previousFile=path.join(root,'browser.json');
  const previous=fs.existsSync(previousFile)?JSON.parse(fs.readFileSync(previousFile)):null;
  const evidence=previous&&previous.sourceDigest===report.sourceDigest&&previous.browser===browser.version()?previous:{browser:browser.version(),sourceDigest:report.sourceDigest,cases:[]};
  try {
    for(const item of report.cases) {
      if(evidence.cases.some(entry=>entry.name===item.name&&entry.sha256===item.optimized.sha256&&entry.passed)) continue;
      const page=await browser.newPage(); const errors=[];
      page.on('pageerror',error=>errors.push(error.message));
      await page.addInitScript(() => {
        const records=[];
        document.addEventListener('DOMContentLoaded',()=>records.push(document.body?.textContent));
        window.readinessRecords=records;
      });
      await page.goto(`http://127.0.0.1:${server.address().port}/${item.name}/index.html`);
      await page.waitForFunction(()=>document.body.dataset.ready==='true'||!!document.body.dataset.error);
      const failure=await page.locator('body').getAttribute('data-error');
      check(!failure,`${item.name}: ${failure}`);
      if(item.features.includes('button')) {
        await page.getByRole('button',{name:'Count 0',exact:true}).click();
        await page.getByRole('button',{name:'Count 1',exact:true}).waitFor();
      } else if(item.features.includes('use-resources')) {
        await page.getByText('resource ready',{exact:true}).waitFor();
      } else if(item.features.includes('use-css')) {
        check(await page.locator('.sample').evaluate(node=>getComputedStyle(node).opacity)==='0.75','CSS parser/lowering was not exercised');
      } else if(item.features.includes('use-i18n')) {
        check((await page.locator('#app').textContent()).includes('baseline'),'Fluent message was not resolved');
      } else if(item.name!=='empty'&&!item.features.includes('minimal')) {
        check((await page.locator('#app').textContent())==='hello','Basic text missing');
      }
      if(item.features.includes('use-ssr')) check((await page.locator('#app').getAttribute('data-ssr')).includes('server output'),'SSR output missing');
      if(item.features.includes('use-hydrate')||item.features.includes('use-stream')) check(await page.evaluate(()=>window.fixture.before.length>0&&window.fixture.before.every(node=>node.isConnected&&document.querySelector('#app').contains(node))),'Hydration replaced the server elements');
      if(item.features.includes('use-stream')) check(await page.locator('#data').count()===0,'Resource hydration did not consume the data block');
      if(item.features.includes('embed-stream-script')) check((await page.locator('#app').getAttribute('data-stream-script')).length>1000,'Script embedding probe did not retain the actual bytes');
      const mounted=await page.locator('#app *').count();
      await page.evaluate(()=>window.fixture.unmount());
      check(await page.locator('#app').evaluate(node=>node.childNodes.length)===0,`${item.name}: cleanup retained DOM nodes`);
      check(errors.length===0,`${item.name}: ${errors.join('; ')}`);
      evidence.cases=evidence.cases.filter(entry=>entry.name!==item.name);
      evidence.cases.push({name:item.name,sha256:item.optimized.sha256,mounted,errors,passed:true});
      fs.writeFileSync(path.join(root,'browser.json'),JSON.stringify(evidence,null,2));
      console.log(`PASS ${item.name} (${mounted} nodes)`);
      await page.close();
    }
  } finally { await browser.close();server.close(); }
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
