async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const errors = [], limits = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() !== 'error' || !message.text().includes('Glory')) return;
      (/RowLimit/.test(message.text()) ? limits : errors).push(message.text());
    });
    const check = (value, message) => { if (!value) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const stats = () => page.evaluate(() => JSON.parse(window.demo.focus_stats()));
    const results = [];
    for (const adaptive of [false, true]) {
      await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready === true);
      await page.evaluate(adaptive => window.demo.focus_mount(adaptive), adaptive); await settle();
      await page.addStyleTag({ content: '.focus-panel{background-image:linear-gradient(red,blue)}' });
      const background = await page.locator('.focus-panel').evaluate(element => ({ color: getComputedStyle(element).backgroundColor, image: getComputedStyle(element).backgroundImage }));
      check(background.color === 'rgb(1, 2, 3)' && background.image.startsWith('linear-gradient'), 'typed color erased stylesheet background image');
      await page.locator('.edit-1').focus(); await settle();
      const before = await stats(), previousErrors = limits.length;
      await page.getByRole('list').evaluate(element => element.style.height = '1000000px');
      await page.waitForFunction(() => document.querySelectorAll('[role=listitem]').length === 0 && JSON.parse(window.demo.focus_stats()).tasks === 0);
      await settle();
      const failed = await stats();
      check(failed.created === before.created && failed.keys - before.keys <= before.rows + 1 && failed.maxIndex === before.maxIndex,
        `oversized viewport read or created its new window: ${JSON.stringify({ before, failed })}`);
      check(failed.created === failed.dropped && failed.tasks === 0 && failed.nodes === 4, 'failed window retained rows/tasks');
      check(limits.length > previousErrors && limits.slice(previousErrors).every(text => text.includes('limit: 4096')), 'missing typed row limit');
      check(await page.getByRole('list').evaluate(element => document.activeElement === element), 'limit disposal did not restore collection focus');
      await page.getByRole('list').evaluate(element => element.style.height = '240px');
      await page.waitForFunction(() => document.querySelectorAll('[role=listitem]').length > 0); await settle();
      const recovered = await stats();
      check(recovered.tasks === recovered.rows && recovered.rows <= 16 && recovered.created - recovered.dropped === recovered.rows, 'valid resize did not recover');
      await page.keyboard.press('End'); await settle();
      check(await page.evaluate(() => document.activeElement.getAttribute('aria-posinset') === '100000000'), 'recovered list is not navigable');
      await page.evaluate(() => window.demo.unmount()); await settle();
      const lifetime = await page.evaluate(() => JSON.parse(window.demo.focus_lifetimes()));
      check(lifetime.created === lifetime.dropped, 'recovery leaked row owners');
      results.push({ adaptive, background, before, failed, recovered, expectedErrors: limits.slice(previousErrors), lifetime });
    }
    check(errors.length === 0, errors.join('\n'));
    return { results, errors };
  } finally { await context.close(); }
}
