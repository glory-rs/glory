const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { chromium } = require(process.env.GLORY_PLAYWRIGHT || '../../tests/playwright/node_modules/playwright');
const directory = process.env.GLORY_ROUTING_SITE || 'D:/BuildCache/glory-routing/site';
const checks = [];
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://localhost');
  const name = url.pathname.startsWith('/assets/') ? url.pathname.slice('/assets/'.length) : (url.searchParams.has('ssr') ? 'ssr.html' : 'csr.html');
  const file = path.resolve(directory, name);
  if (!file.startsWith(path.resolve(directory) + path.sep)) { res.writeHead(400).end(); return; }
  res.setHeader('Content-Type', name.endsWith('.wasm') ? 'application/wasm' : name.endsWith('.js') ? 'text/javascript' : 'text/html');
  if (!fs.existsSync(file)) { res.writeHead(404).end(); return; }
  res.end(fs.readFileSync(file));
});
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const executable = process.env.GLORY_CHROME || (process.platform === 'win32' && [
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
  ].find(file => fs.existsSync(file)));
  const browser = await chromium.launch({ headless: true, ...(executable ? { executablePath: executable } : {}) });
  try {
    for (const { ssr, late } of [{ ssr: false, late: false }, { ssr: true, late: false }, { ssr: true, late: true }]) {
      console.error(`routing case: ssr=${ssr}, late=${late}`);
      const page = await browser.newPage();
      const errors = [];
      page.on('pageerror', error => { errors.push(error.message); console.error('browser:', error.message); });
      page.on('console', message => { if (message.type() === 'error') { errors.push(message.text()); console.error('browser:', message.text()); } });
      if (ssr) await page.route('**/assets/glory_routing_demo.js', async route => {
        await page.evaluate(late => {
          const input = document.querySelector('.shell-editor');
          input.value = 'typed before hydration'; input.focus(); input.setSelectionRange(2, 6);
          window.initialInput = input;
          window.history.replaceState({ otherApplication: 'retained' }, '', late ? '/page/2?tab=late' : window.location.href);
        }, late);
        await route.continue();
      });
      await page.goto(`${base}/page/1${ssr ? '?ssr=1' : ''}`);
      await page.waitForFunction(() => window.ready);
      await page.waitForFunction(id => document.querySelector('.route-value')?.textContent.includes(`id=${id}`), late ? 2 : 1);
      if (ssr) {
        assert.equal(await page.evaluate(() => document.querySelector('.shell-editor') === window.initialInput), true);
        assert.equal(await page.locator('.shell-editor').inputValue(), 'typed before hydration');
        assert.deepEqual(await page.locator('.shell-editor').evaluate(input => [input.selectionStart, input.selectionEnd]), [2, 6]);
        assert.equal(await page.evaluate(() => window.history.state.otherApplication), 'retained');
      }
      await page.evaluate(() => { window.retainedInput = document.querySelector('.shell-editor'); });
      await page.getByRole('link', { name: 'Two', exact: true }).click();
      await page.waitForFunction(() => location.pathname === '/page/2' && !location.search && !location.hash && document.querySelector('.route-value')?.textContent.includes('id=2;tab=;hash='));
      assert.equal(await page.evaluate(() => document.querySelector('.shell-editor') === window.retainedInput), true);
      await page.evaluate(() => window.demo.navigate('/page/2?tab=edit#details', false, false));
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('tab=edit;hash=details')).catch(async error => {
        console.error(await page.evaluate(() => ({ href: location.href, value: document.querySelector('.route-value')?.textContent, history: history.state })));
        throw error;
      });
      await page.waitForFunction(() => window.scrollY > 1000);
      await page.goBack();
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=2;tab=;hash='));
      await page.evaluate(() => window.scrollTo(0, 400));
      await page.waitForFunction(() => window.scrollY >= 390 && window.scrollY <= 410);
      await page.waitForTimeout(70);
      await page.evaluate(() => window.demo.navigate('/page/1', false, false));
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=1'));
      await page.goBack();
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=2'));
      await page.waitForFunction(() => window.scrollY >= 390 && window.scrollY <= 410);
      await page.evaluate(() => window.demo.navigate('/page/2', false, true));
      await page.waitForTimeout(70);
      await page.evaluate(() => window.scrollTo(0, 700));
      await page.waitForTimeout(70);
      await page.evaluate(() => window.demo.navigate('/page/1', false, false));
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=1'));
      await page.goBack();
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=2'));
      await page.waitForFunction(() => window.scrollY >= 690 && window.scrollY <= 710);
      await page.goBack();
      await page.waitForFunction(() => window.scrollY >= 390 && window.scrollY <= 410);
      const cancelledRestore = await page.evaluate(async () => {
        window.demo.navigate('/page/2#missing-anchor', false, false);
        await new Promise(requestAnimationFrame);
        window.demo.navigate('/page/2?tab=cancel', false, false);
        // Let both the old anchor retry and the new navigation's effects run.
        for (let frame = 0; frame < 12; frame++) await new Promise(requestAnimationFrame);
        return { y: window.scrollY, search: location.search, hash: location.hash };
      });
      assert.equal(cancelledRestore.search, '?tab=cancel');
      assert.equal(cancelledRestore.hash, '');
      assert.ok(cancelledRestore.y >= 390 && cancelledRestore.y <= 410,
        `superseded anchor restoration changed scroll to ${cancelledRestore.y}`);
      await page.evaluate(() => {
        const anchor = document.createElement('a'); anchor.href = '/page/3'; anchor.dataset.gloryRoute = ''; anchor.textContent = 'raw route'; document.querySelector('#app').append(anchor);
      });
      await page.getByRole('link', { name: 'raw route' }).click();
      await page.waitForFunction(() => document.querySelector('.route-value')?.textContent.includes('id=3'));
      await page.evaluate(() => window.demo.navigate('/missing', false, false));
      await page.getByText('Not found: /missing', { exact: false }).waitFor();
      await page.evaluate(() => window.demo.stop());
      assert.equal(await page.evaluate(() => window.history.scrollRestoration), 'auto');
      assert.deepEqual(errors, []);
      checks.push(`${late ? 'SSR with changed pre-WASM URL' : ssr ? 'SSR hydration' : 'CSR'}: layout identity, dirty editor, query/hash, history, duplicate URL scroll, cancelled anchor restoration, raw anchor, fallback, teardown`);
      await page.close();
    }
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('**/assets/glory_routing_demo.js', async route => {
      // The native fragment lookup runs before CSR has created the anchor.
      assert.equal(await page.locator('#details').count(), 0);
      await new Promise(resolve => setTimeout(resolve, 250));
      await route.continue();
    });
    await page.goto(`${base}/page/1#details`);
    await page.waitForFunction(() => window.ready);
    await page.waitForFunction(() => window.scrollY > 1000);
    assert.equal(await page.locator('#details').count(), 1);
    assert.deepEqual(errors, []);
    checks.push('CSR initial fragment: delayed WASM still scrolls to the mounted anchor');
    await page.close();
    console.log(JSON.stringify({ checks, passed: checks.length }, null, 2));
  } finally { await browser.close(); server.close(); }
})().catch(error => { console.error(error); server.close(); process.exitCode = 1; });
