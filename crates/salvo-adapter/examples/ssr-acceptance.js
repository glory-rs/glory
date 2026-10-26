// Playwright callback; run against `cargo run -p glory-salvo --example ssr --features stream`.
async (hostPage) => {
  const context = await hostPage.context().browser().newContext();
  try {
    const page = await context.newPage();
    const check = (value, message) => { if (!value) throw new Error(message); };
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && /Glory|Content Security Policy|Refused/.test(message.text())) errors.push(message.text());
    });
    await page.addInitScript(() => {
      window.salvoFrames = [];
      for (const type of ['glory:stream-patch', 'glory:stream-ready', 'glory:stream-error']) {
        document.addEventListener(type, () => window.salvoFrames.push({
          type, time: performance.now(), text: document.querySelector('#app')?.textContent,
        }), true);
      }
    });
    const response = await page.goto('http://127.0.0.1:5800/stream', { waitUntil: 'commit' });
    check(response.status() === 200, 'HTTP response failed');
    check((await response.allHeaders())['content-security-policy'].includes("script-src 'self'"), 'missing script CSP');
    await page.waitForSelector('#app');
    const shell = await page.locator('#app').textContent();
    check(shell.includes('Waiting for slow'), 'shell was withheld until all resources completed');
    await page.waitForFunction(() => document.querySelector('#app')?.textContent.includes('fast result arrived'));
    const middle = await page.locator('#app').textContent();
    check(middle.includes('Waiting for slow'), 'independent fast result was buffered until slow completion');
    await page.waitForFunction(() => document.querySelector('#app')?.dataset.glyStreamState === 'ready');
    const end = await page.evaluate(() => ({
      text: document.querySelector('#app').textContent,
      pending: document.querySelector('#app').hasAttribute('data-gly-stream-pending'),
      active: GloryStream.active(),
      seeds: JSON.parse(document.querySelector('#request-resources').textContent),
      frames: window.salvoFrames,
    }));
    check(end.text.includes('slow result arrived') && !end.text.includes('Waiting for'), 'patches did not update the actual DOM');
    check(!end.pending && end.active === 0, 'End did not release the stream controller');
    check(end.frames.filter(frame => frame.type === 'glory:stream-patch').length === 2, 'expected two incremental patches');
    check(end.frames.at(-1).type === 'glory:stream-ready', 'missing successful End');
    check(JSON.stringify(end.seeds).includes('fast result') && JSON.stringify(end.seeds).includes('slow result'), 'resource seeds missing');
    await page.goto('http://127.0.0.1:5800/plain');
    check(await page.getByRole('heading', { name: 'Glory + Salvo SSR' }).count() === 1, 'ordinary SSR did not render');
    check(errors.length === 0, errors.join('\n'));
    return { shell, middle, end, errors };
  } finally {
    await context.close();
  }
}
