// Compare independently built Plan batch variants with the unchanged rows
// harness. Per-operation timing stays in the page; CPU counters cover a round.
'use strict';
const fs = require('node:fs'), path = require('node:path'), http = require('node:http');
const cp = require('node:child_process'), crypto = require('node:crypto'), assert = require('node:assert/strict');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const root = path.resolve(option('--root', 'D:/BuildCache/glory-v2-plan-batch'));
const out = path.resolve(option('--out', root + '/comparison-' + Date.now()));
const rounds = Number(option('--rounds', '4'));
assert(Number.isInteger(rounds) && rounds >= 1 && rounds <= 20);
for (const dir of [root, out]) assert(dir.replaceAll('\\', '/').toLowerCase().startsWith('d:/buildcache/'));
process.env.TEMP = process.env.TMP = path.join(root, 'tmp');
fs.mkdirSync(process.env.TEMP, { recursive: true });
fs.mkdirSync(out);
fs.copyFileSync(__filename, path.join(out, 'driver.cjs'));
const { chromium } = require('../tests/playwright/node_modules/playwright');
const routes = { before: 'before/site/glory', after: 'after/site/glory', dioxus: 'before/site/dioxus' };
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const evidence = {
  schema: 1, contract: 'semantic-rows-v1', rounds: [],
  primary_metric: 'click_to_two_raf_ms',
  launch: { headless: true, args: ['--disable-gpu'], fresh_browser_per_variant: true },
  artifacts: Object.fromEntries(Object.entries(routes).map(([name, route]) =>
    [name, { wasm: sha(path.join(root, route, 'app_bg.wasm')) }]))
};
const save = () => fs.writeFileSync(path.join(out, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
const environment = () => JSON.parse(cp.execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
  "[pscustomobject]@{load=(Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor | Where-Object Name -eq _Total).PercentProcessorTime; builds=@(Get-Process -Name cargo,rustc -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU)} | ConvertTo-Json -Depth 4 -Compress"
], { encoding: 'utf8', windowsHide: true, timeout: 30000 }));
const bounded = (promise, ms, label) => {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(Error(label + ' timed out')), ms);
  })]).finally(() => clearTimeout(timer));
};
const server = http.createServer((req, res) => {
  try {
    let file = path.resolve(root, '.' + new URL(req.url, 'http://localhost').pathname);
    assert(file.startsWith(root + path.sep));
    if (fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' :
      file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : 'text/html');
    fs.createReadStream(file).pipe(res);
  } catch { res.statusCode = 404; res.end(); }
});
async function measure(page) {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  await cdp.send('HeapProfiler.collectGarbage');
  const metrics = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
  const before = await metrics(), started = new Date().toISOString();
  const result = await page.evaluate(() => window.__BENCH__.runAll());
  const after = await metrics();
  await cdp.detach();
  return {
    ...result, started, finished: new Date().toISOString(), errors,
    aggregate_metrics: Object.fromEntries(['ScriptDuration', 'LayoutDuration', 'RecalcStyleDuration', 'TaskDuration']
      .map(field => [field, (after[field] - before[field]) * 1000]))
  };
}
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const outcomes = new Map();
  try {
    for (let round = 0; round < rounds; round++) {
      const names = round % 2 ? Object.keys(routes).reverse() : Object.keys(routes);
      for (const name of names) {
        const browser = await chromium.launch({ headless: true, args: ['--disable-gpu'],
          executablePath: option('--browser', 'C:/Program Files/Google/Chrome/Application/chrome.exe') });
        try {
          evidence.browser = browser.version();
          const context = await browser.newContext({ viewport: { width: 1280, height: 900 },
            deviceScaleFactor: 1, locale: 'en-US', colorScheme: 'light' });
          const page = await context.newPage();
          page.setDefaultTimeout(120000);
          await page.goto(`http://127.0.0.1:${server.address().port}/${routes[name]}/`);
          await page.bringToFront();
          await page.waitForFunction(() => window.__BENCH_READY__ && window.__BENCH__);
          cp.execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File',
            path.join(__dirname, 'scenarios/pin-browser.ps1'), '-ProfileRoot', process.env.TEMP,
            '-Out', path.join(out, 'affinity.jsonl')], { windowsHide: true, timeout: 30000 });
          const loadBefore = environment(), result = await bounded(measure(page), 600000, name);
          const loadAfter = environment();
          assert.deepEqual(result.errors, []);
          for (const row of result.results) {
            assert.equal(row.samples.length, 12);
            row.samples.forEach((sample, index) => {
              const key = row.key + ':' + index, value = [sample.count, sample.sha256];
              if (outcomes.has(key)) assert.deepEqual(value, outcomes.get(key)); else outcomes.set(key, value);
            });
          }
          evidence.rounds.push({ round, name, loadBefore, loadAfter, ...result });
          save();
          console.log(`Measured ${round + 1}/${rounds}: ${name}`);
        } finally { await bounded(browser.close(), 30000, 'browser close'); }
      }
    }
    evidence.completed = new Date().toISOString();
    save();
  } catch (error) { evidence.failure = String(error.stack); save(); throw error; }
  finally { server.close(); }
})().catch(error => { console.error(error); process.exit(1); });
