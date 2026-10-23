async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 800, height: 600 } });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    await page.goto('http://127.0.0.1:8788/');
    await page.waitForFunction(() => window.ready === true);
    const probe = await page.evaluate(() => {
      window.demo.target_mount();
      return JSON.parse(window.demo.event_probe());
    });
    assert(probe.rejected === 13 && probe.malformedCallbacks === 0 && probe.validCallbacks === 1, JSON.stringify(probe));
    assert(probe.slotReuse && probe.staleIgnored && probe.nodes === 8, 'origin identity/cleanup mismatch');
    await page.evaluate(() => window.demo.target_query());
    await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).responses.length === 32);
    assert((await page.evaluate(() => JSON.parse(window.demo.target_stats()))).queries === 0, 'valid queries did not recover');
    await page.locator('.target-button').click();
    await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).activations === 1);
    await page.locator('.target-input').fill('recovered🙂');
    assert(await page.locator('.target-input').inputValue() === 'recovered🙂', 'valid editing did not recover');
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#app [data-gly-kind]').count() === 0, 'DOM retained after cleanup');
    assert(errors.length === 0, JSON.stringify(errors));
    return { probe, queriesRecovered: 32, editingRecovered: true, cleanup: true, errors };
  } finally { await context.close(); }
}
