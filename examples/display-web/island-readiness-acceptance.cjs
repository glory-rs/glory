'use strict';
const fs = require('node:fs'), path = require('node:path'), http = require('node:http'), assert = require('node:assert/strict');
const { chromium } = require(process.env.GLORY_PLAYWRIGHT || path.resolve(__dirname, '../../tests/playwright/node_modules/playwright'));
const site = path.resolve(process.env.GLORY_ISLAND_SITE || 'D:/BuildCache/glory-semantic-ui/island-readiness-site');
const counts = (process.env.GLORY_ISLAND_COUNTS || '1,32,64').split(',').map(Number);
const evidence = { build: 'debug wasm32-unknown-unknown', cases: [], checks: [], errors: [] };
const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://localhost');
    if (url.pathname === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    if (url.pathname === '/') {
        res.setHeader('Content-Type', 'text/html');
        res.end('<!doctype html><html><head><meta charset="utf-8"><style>.editor{display:block}</style></head><body><div id="islands"></div><script type="module">try{const app=await import("/app.js");await app.default();window.app=app;app.island_mount(Number(new URL(location.href).searchParams.get("count")));window.__READY__=true;}catch(error){window.__FAILURE__=String(error);throw error;}</script></body></html>');
        return;
    }
    try {
        const file = path.resolve(site, '.' + decodeURIComponent(url.pathname));
        assert(file.startsWith(site + path.sep));
        res.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
        fs.createReadStream(file).on('error', () => { res.statusCode = 404; res.end(); }).pipe(res);
    } catch { res.statusCode = 404; res.end(); }
});
(async () => {
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const browser = await chromium.launch({ headless: true, executablePath: process.env.GLORY_CHROME || 'C:/Program Files/Google/Chrome/Application/chrome.exe' });
    try {
        evidence.browser = browser.version();
        for (const count of counts) {
            const page = await browser.newPage();
            page.on('pageerror', error => evidence.errors.push(error.stack || String(error)));
            page.on('console', message => { if (message.type() === 'error') evidence.errors.push(message.text()); });
            const settle = () => page.evaluate(async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); });
            const stats = () => page.evaluate(() => JSON.parse(app.island_stats()));
            await page.goto(`http://127.0.0.1:${server.address().port}/?count=${count}`);
            await page.waitForFunction(() => window.__READY__ || window.__FAILURE__);
            const admissionFailure = await page.evaluate(() => window.__FAILURE__);
            if (admissionFailure !== undefined) evidence.admissionFailure = { count, error: admissionFailure };
            assert.equal(admissionFailure, undefined, `mount ${count}`);
            await settle();
            const initial = await stats();
            assert.equal(initial.islands, count); assert.equal(initial.handles, count);
            assert.equal(await page.locator('.editor').count(), count);
            await page.evaluate(() => app.island_raw_updates(20)); await settle();
            const measurements = await page.evaluate(() => {
                const before = JSON.parse(app.island_stats()), samples = [];
                for (let repeat = 0; repeat < 7; repeat++) {
                    const start = performance.now(); app.island_raw_updates(200);
                    samples.push(performance.now() - start);
                }
                return { before, after: JSON.parse(app.island_stats()), samples, iterationsPerSample: 200 };
            });
            assert.equal(measurements.after.island_flushes - measurements.before.island_flushes, 0);
            assert.equal(measurements.after.raw - measurements.before.raw, 1400);
            assert.equal(await page.locator('.raw').textContent(), String(measurements.after.raw));
            await settle();
            let before = await stats();
            await page.evaluate(() => app.island_shared_update());
            let after = await stats();
            assert.equal(after.island_flushes - before.island_flushes, count, 'shared runtime update must flush every affected island');
            assert.deepEqual(await page.locator('.shared').allTextContents(), Array(count).fill('1'));
            await settle();
            const index = Math.floor(count / 2);
            before = await stats();
            await page.evaluate(index => app.island_local_update(index, 'programmatic'), index);
            after = await stats();
            assert.equal(after.island_flushes - before.island_flushes, 1);
            assert.equal(await page.locator(`.editor-${index}`).inputValue(), 'programmatic');
            await settle();
            before = await stats();
            after = await page.evaluate(index => {
                const input = document.querySelector(`.editor-${index}`);
                input.value = 'host input'; input.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText', data: 'host input' }));
                app.island_flush();
                return JSON.parse(app.island_stats());
            }, index);
            assert.equal(after.island_flushes - before.island_flushes, 1, 'pending host turn must enter the island snapshot');
            assert.equal(await page.locator(`.editor-${index}`).inputValue(), 'host input');
            assert.equal(await page.evaluate(index => app.island_value(index), index), 'host input');
            await settle();
            const hostInputPumps = (await stats()).island_flushes - before.island_flushes;
            assert.equal(hostInputPumps, 2, 'only the active island receives the coalesced follow-up pump');
            // Exercise Chrome-generated keyboard input through the independent RAF path too.
            await page.locator(`.editor-${index}`).fill('real browser edit'); await settle();
            assert.equal(await page.locator(`.editor-${index}`).inputValue(), 'real browser edit');
            assert.equal(await page.evaluate(index => app.island_value(index), index), 'real browser edit');
            // Use the highest ID: earlier idle islands must commit before the
            // explicit flush returns, not merely in the next Bridge microtask.
            const sharedBarrier = await page.evaluate(count => {
                document.querySelector(`.global-button-${count - 1}`).click();
                app.island_flush();
                return { count, labels: [...document.querySelectorAll('.shared')].map(node => node.textContent) };
            }, count);
            (evidence.hostSharedBarriers ||= []).push(sharedBarrier);
            assert.deepEqual(sharedBarrier.labels, Array(count).fill('2'), 'host-to-shared updates must synchronously commit earlier idle islands');
            await settle();
            assert.equal((await stats()).shared, 2);
            assert.deepEqual(await page.locator('.shared').allTextContents(), Array(count).fill('2'));
            await page.evaluate(index => app.island_dirty_query(index), index); await settle();
            after = await stats();
            assert.equal(after.replies, 1); assert.equal(after.failures, 0);
            assert.equal(await page.locator(`.editor-${index}`).inputValue(), 'query barrier');
            for (let round = 0; round < 2; round++) {
                await page.evaluate(() => { window.retiredEditor = document.querySelector('.editor'); app.island_toggle(); }); await settle();
                const hidden = await stats(); assert.equal(hidden.islands, 0); assert.equal(hidden.handles, 0);
                await page.evaluate(() => { retiredEditor.value = 'retired event'; retiredEditor.dispatchEvent(new Event('input', { bubbles: true })); app.island_raw_updates(1); }); await settle();
                assert.equal((await stats()).islands, 0);
                await page.evaluate(() => app.island_toggle()); await settle();
                const restored = await stats();
                assert.equal(restored.islands, count); assert.equal(restored.handles, count);
                assert.equal(restored.nodes, initial.nodes); assert.equal(restored.owners, initial.owners);
                assert.deepEqual(await page.locator('.shared').allTextContents(), Array(count).fill('2'));
                assert.equal(await page.locator(`.editor-${index}`).inputValue(), 'initial');
            }
            let budgetRejection;
            const closed = await page.evaluate(count => JSON.parse(count === 64 ? app.island_reject_extra_and_close() : app.island_close()), count).then(result => {
                if (count !== 64) return result;
                budgetRejection = result;
                assert.equal(result.typedHostRejection, true); assert.equal(result.actualIslandRejection, true);
                return result.closed;
            }); await settle();
            assert.deepEqual(closed, { islands: 0, handles: 0, nodes: 0, owners: 1 });
            assert.equal(await page.locator('#islands > *').count(), 0);
            const sorted = measurements.samples.slice().sort((a, b) => a - b);
            evidence.cases.push({ count, rawIslandPumps: 0, sharedIslandPumps: count, localIslandPumps: 1, synchronousHostInputIslandPumps: 1, settledHostInputIslandPumps: hostInputPumps, samplesMs: measurements.samples, medianMsPer200Updates: sorted[3], medianUsPerUpdate: sorted[3] * 5, initial, closed, budgetRejection });
            await page.close();
        }
        assert.deepEqual(evidence.errors, []);
        evidence.checks.push(`${counts.join('/')} live islands: raw field updates pump zero idle islands; shared and single-island presentations commit synchronously`, 'pending host input, real Chrome editing, dirty Bounds query barriers, two disposal/restoration cycles and final owner/DOM release pass');
        evidence.boundaries = 'The existing DocumentLayers budget permits 64 live WebApps per Document, including Widget islands and other WebApps; a 65th island is rejected. 100/1000 islands in one Document are unsupported (the attempted 100-island failure is retained separately). Readiness still scans O(live islands); island_flushes counts actual explicit display-host pumps, not independent RAF callbacks. Debug elapsed times are local observations, not release throughput or a before/after speedup.';
        evidence.passed = true;
        console.log(JSON.stringify(evidence));
    } finally {
        fs.writeFileSync(path.join(site, 'island-readiness-evidence.json'), JSON.stringify(evidence, null, 2));
        await browser.close(); await new Promise(resolve => server.close(resolve));
    }
})().catch(error => { console.error(error); server.close(); process.exitCode = 1; });
