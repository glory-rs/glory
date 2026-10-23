async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    const check = (value, message) => { if (!value) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const stats = () => page.evaluate(() => JSON.parse(window.demo.focus_stats()));
    const results = [];
    for (const adaptive of [false, true]) {
      await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready === true);
      await page.evaluate(adaptive => window.demo.focus_mount(adaptive), adaptive); await settle();
      await page.locator('.edit-1').fill('editing survives');
      await page.evaluate(() => {
        const input = document.querySelector('.edit-1'); input.focus(); input.setSelectionRange(2, 9);
        window.original = input; window.setterWrites = 0;
        const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
        Object.defineProperty(input, 'value', { configurable: true, get() { return descriptor.get.call(this); }, set(value) { window.setterWrites++; return descriptor.set.call(this, value); } });
      });
      await settle();
      let maxRows = 0, maxSlots = 0, maxGraphSlots = 0, maxKeyReads = 0;
      for (let visit = 1; visit <= 40; visit++) {
        const before = await stats();
        await page.evaluate(visit => window.demo.focus_jump(((visit * 7919) % 90_000) * 28_000), visit); await settle();
        const current = await stats();
        maxRows = Math.max(maxRows, current.rows); maxSlots = Math.max(maxSlots, current.slots); maxGraphSlots = Math.max(maxGraphSlots, current.graphSlots);
        maxKeyReads = Math.max(maxKeyReads, current.keys - before.keys);
        check(current.pinned === 1 && current.rows === current.end - current.start + 1, 'window plus one pin');
        check(current.rows <= 16 && current.tasks === current.rows && current.created - current.dropped === current.rows, 'bounded real owners/tasks');
        const retained = await page.evaluate(() => [document.activeElement === window.original, window.original.isConnected, window.original.value, window.original.selectionStart, window.original.selectionEnd]);
        check(JSON.stringify(retained) === JSON.stringify([true, true, 'editing survives', 2, 9]), `lost focused edit ${JSON.stringify(retained)}`);
        // Exercise a real native physical scroll after each segmented seek.
        await page.locator('[role=list]').evaluate(element => { element.scrollTop += 1; }); await settle();
      }
      // Composition remains on the same editing object while it is offscreen.
      await page.evaluate(() => {
        const input = window.original;
        input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, '组合输入中');
        input.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true }));
        window.demo.focus_jump(1_250_000_000);
      }); await settle();
      check(await page.evaluate(() => document.activeElement === window.original && window.original.value === '组合输入中'), 'composition lost offscreen');
      await page.evaluate(() => {
        const input = window.original;
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, 'editing survives');
        input.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
        input.setSelectionRange(2, 9);
      }); await settle();
      await page.evaluate(() => window.demo.focus_prepend()); await settle();
      check(await page.locator('.label-1').innerText() === 'key 1 at 10001', 'same row receives relocated index');
      check(await page.evaluate(() => document.activeElement === window.original && window.original.isConnected), 'prepend retained focused identity');
      // Returning it into the window must not mount a second copy or overwrite draft.
      await page.evaluate(() => window.demo.focus_jump(10_001 * 28)); await settle();
      check(await page.locator('.edit-1').count() === 1, 'duplicate focused row');
      check((await stats()).pinned === 0, 'returning pin rejoins normal window');
      // Rapid native A→B→A transitions in one frame must end at A.
      await page.evaluate(() => {
        const other = document.querySelector('.edit-2'); other.focus(); window.original.focus(); window.original.setSelectionRange(2, 9);
      }); await settle();
      await page.evaluate(() => window.demo.focus_jump(1_500_000_000)); await settle();
      check((await stats()).pinned === 1 && await page.evaluate(() => document.activeElement === window.original), 'rapid transition changed final pin');
      const writes = await page.evaluate(() => window.setterWrites); check(writes === 0, 'framework rewrote preserved input');
      await page.locator('.outside-focus').focus(); await settle();
      check(await page.evaluate(() => !window.original.isConnected), 'blur did not dispose offscreen row');
      check((await stats()).pinned === 0, 'blur retained pin metadata');
      // Repeat with deletion of an offscreen focused key.
      await page.evaluate(() => window.demo.focus_jump(10_001 * 28)); await settle();
      await page.locator('.edit-1').focus(); await settle();
      await page.evaluate(() => { window.deleted = document.querySelector('.edit-1'); window.demo.focus_jump(1_000_000_000); }); await settle();
      await page.evaluate(() => window.demo.focus_remove()); await settle();
      check(await page.evaluate(() => !window.deleted.isConnected), 'deleted focused row remained mounted');
      check((await stats()).pinned === 0, 'deleted row retained pin metadata');
      const final = await stats();
      await page.evaluate(() => window.demo.unmount()); await settle();
      const lifetime = await page.evaluate(() => JSON.parse(window.demo.focus_lifetimes()));
      check(lifetime.created === lifetime.dropped && await page.locator('[role=listitem]').count() === 0, 'unmount leaked rows');
      results.push({ adaptive, maxRows, maxSlots, maxGraphSlots, maxKeyReads, writes, final, lifetime });
    }
    check(errors.length === 0, errors.join('\n'));
    return { results, errors };
  } finally { await context.close(); }
}
