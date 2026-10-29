'use strict';
const fs = require('node:fs'), path = require('node:path'), http = require('node:http'), assert = require('node:assert/strict');
const { chromium } = require('D:/BuildCache/glory-v2-dioxus-20261028/tools/node_modules/playwright');
const site = path.resolve(process.env.GLORY_WIDGET_SITE || 'D:/BuildCache/glory-widget-dsl/site');
const evidencePath = path.join(path.dirname(site), 'unified-evidence.json');
const errors = [], evidence = { checks: [] };
const server = http.createServer((req, res) => {
    if (req.url === '/favicon.ico') { res.statusCode = 204; res.end(); return; }
    try {
        let file = path.resolve(site, '.' + new URL(req.url, 'http://localhost').pathname);
        assert(file.startsWith(site + path.sep));
        if (fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
        res.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'text/javascript' : 'text/html');
        fs.createReadStream(file).pipe(res);
    } catch { res.statusCode = 404; res.end(); }
});
(async () => {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const browser = await chromium.launch({headless: true, executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe'});
    try {
        const page = await browser.newPage();
        page.on('pageerror', error => errors.push(error.stack));
        page.on('console', message => { if (message.type() === 'error' && !message.text().includes('favicon')) errors.push(message.text()); });
        await page.goto(`http://127.0.0.1:${server.address().port}/index.html`);
        await page.waitForFunction(() => window.__READY__);
        const settle = () => page.evaluate(async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); });
        const stats = () => page.evaluate(() => JSON.parse(window.app.unified_stats()));
        const click = async selector => { await page.locator('#unified ' + selector).click(); await settle(); };
        const initial = await stats();
        await page.evaluate(() => { window.__row = document.querySelector('#unified .row-button'); window.__row.focus(); window.app.unified_swap(); });
        assert.deepEqual(await page.locator('#unified .row-button').allTextContents(), ['row 2', 'row 1']);
        assert(await page.evaluate(() => window.__row === document.querySelectorAll('#unified .row-button')[1] && document.activeElement === window.__row));
        evidence.checks.push('keyed semantic Widget movement preserves DOM identity and focus');
        assert.equal(await page.locator('#unified .shared').textContent(), '0');
        await page.evaluate(() => { window.__full = document.querySelector('#unified .full'); window.__owned = document.querySelector('#unified .owned'); });
        await click('.full'); await click('.imported'); await click('.custom-button'); await click('.nested');
        assert.equal(await page.locator('#unified .shared').textContent(), '4');
        assert.equal(await page.locator('#unified .full').textContent(), 'count 4');
        assert.equal(await page.locator('#unified .local').textContent(), '1');
        assert.equal(await page.locator('#unified .raw-child > span').textContent(), 'inside semantic widget');
        assert.equal(await page.locator('#unified .content-widget').textContent(), 'content');
        assert(await page.evaluate(() => window.app.unified_reject_nested_button()));
        evidence.checks.push('path, import, custom Widget and bidirectional HTML/Widget nesting share reactive state');
        await click('.dismiss');
        assert.equal(await page.locator('#unified .branch').count(), 0);
        assert.equal((await stats()).drops, 1);
        const hidden = await stats();
        await page.evaluate(() => window.__owned.click()); await settle();
        assert.equal((await stats()).count, 4);
        await page.evaluate(() => window.app.unified_toggle()); await settle();
        assert.equal(await page.locator('#unified .local').textContent(), '0');
        assert(await page.evaluate(() => window.__full === document.querySelector('#unified .full')));
        for (let n = 0; n < 10; n++) {
            await page.evaluate(() => window.app.unified_toggle()); await settle();
            const now = await stats(); assert.equal(now.owners, hidden.owners); assert.equal(now.nodes, hidden.nodes);
            await page.evaluate(() => window.app.unified_toggle()); await settle();
            const live = await stats(); assert.equal(live.owners, initial.owners); assert.equal(live.nodes, initial.nodes);
        }
        evidence.checks.push('conditional disposal removes handlers and captures, preserves siblings, resets local state and keeps live graph bounded');
        evidence.closed = await page.evaluate(() => JSON.parse(window.app.unified_close())); await settle();
        assert.equal(evidence.closed.nodes, 0); assert.equal(evidence.closed.owners, 1);
        assert.equal(await page.locator('#unified > *').count(), 0);
        assert.deepEqual(errors, []);
        evidence.checks.push('close releases every reactive node and widget owner (only the Runtime sentinel remains)');
        evidence.browser = browser.version(); evidence.passed = true;
        console.log(JSON.stringify(evidence));
    } finally { fs.writeFileSync(evidencePath, JSON.stringify({...evidence, errors}, null, 2)); await browser.close(); server.close(); }
})().catch(error => { console.error(error); server.close(); process.exitCode = 1; });
