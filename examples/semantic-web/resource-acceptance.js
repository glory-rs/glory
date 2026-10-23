async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const check = (condition, message) => { if (!condition) throw new Error(message); };
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready);
    await page.evaluate(() => window.demo.resource_mount());
    const settle = async () => {
      let deadline;
      try { await Promise.race([page.evaluate(() => new Promise(resolve => { let frames = 4; const next = () => --frames ? requestAnimationFrame(next) : resolve(); requestAnimationFrame(next); })), new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error('resource frames missing for 5s')), 5000); })]); }
      finally { clearTimeout(deadline); }
    };
    const stats = () => page.evaluate(() => JSON.parse(window.demo.resource_stats()));
    const label = () => page.locator('.resource-main').innerText();
    await settle();
    const initial = await stats();
    const rows = await page.locator('.resource-row').count();
    check(await label() === 'seed:7', 'matching seed is ready without loading');
    check(initial.pending === rows && initial.tasks === rows, JSON.stringify(initial));
    check(await page.evaluate(() => window.demo.resource_finish(7, false)) === 0, 'seed did not start a request');
    await page.evaluate(() => window.demo.resource_key(8)); await settle();
    check(await label() === 'pending:8', 'key change publishes pending');
    await page.evaluate(() => window.demo.resource_key(9)); await settle();
    check(await page.evaluate(() => window.demo.resource_finish(8, false)) === 0, 'old request was really dropped');
    check(await page.evaluate(() => window.demo.resource_finish(9, false)) === 1, 'current request resolves');
    await settle(); check(await label() === 'loaded:9', 'async completion updates without manual flush');
    await page.evaluate(() => window.demo.resource_refresh()); await settle();
    check(await page.evaluate(() => window.demo.resource_finish(9, true)) === 1, 'refresh starts same key');
    await settle(); check(await label() === 'failed:error:9', 'business failure is rendered');
    await page.evaluate(() => window.demo.resource_refresh()); await settle();
    await page.evaluate(() => window.demo.resource_cancel()); await settle();
    check(await label() === 'idle', 'cancel clears result and remains idle');
    check(await page.evaluate(() => window.demo.resource_finish(9, false)) === 0, 'cancelled future removed');
    let maxTasks = 0, maxSlots = 0, maxRows = 0, maxNodes = 0;
    for (let i = 1; i <= 80; i++) {
      await page.evaluate(offset => window.demo.resource_seek(offset), ((i * 7919) % 90_000) * 24_000); await settle();
      const state = await stats(); const count = await page.locator('.resource-row').count();
      maxTasks = Math.max(maxTasks, state.tasks); maxSlots = Math.max(maxSlots, state.slots);
      maxRows = Math.max(maxRows, count); maxNodes = Math.max(maxNodes, state.nodes);
      check(state.pending === count && state.tasks === count && state.registrations === count, `only mounted rows have tasks/registrations: ${JSON.stringify(state)}`);
      check(count <= 15 && state.slots < 200 && state.nodes < 40, `bounded work: ${JSON.stringify(state)}`);
    }
    const beforeUnmount = await stats();
    check(beforeUnmount.cancelled > 900, 'scroll churn actually cancelled requests');
    await page.evaluate(() => window.demo.unmount()); await settle();
    const afterUnmount = await stats();
    check(afterUnmount.pending === 0 && afterUnmount.tasks === 0 && afterUnmount.graph === 0 && afterUnmount.registrations === 0, JSON.stringify(afterUnmount));
    check(afterUnmount.started === afterUnmount.finished + afterUnmount.cancelled, 'every future finished or dropped');
    check(await page.locator('[data-gly-node]').count() === 0, 'DOM released');
    check(errors.length === 0, errors.join('\n'));
    return { initial, maxTasks, maxRows, maxSlots, maxNodes, beforeUnmount, afterUnmount, errors };
  } finally { await context.close(); }
}
