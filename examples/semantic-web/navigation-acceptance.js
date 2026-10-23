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
    const focused = () => page.evaluate(() => {
      const item = document.activeElement, list = document.querySelector('[role=list]');
      const a = item.getBoundingClientRect(), b = list.getBoundingClientRect();
      const css = getComputedStyle(item);
      return { role: item.getAttribute('role'), index: Number(item.getAttribute('aria-posinset')) - 1,
        size: item.getAttribute('aria-setsize'), tabindex: item.getAttribute('tabindex'),
        focusVisible: item.matches(':focus-visible'), outline: css.outlineStyle,
        visible: a.bottom > b.top && a.top < b.bottom, fullyVisible: a.top >= b.top - 1 && a.bottom <= b.bottom + 1,
        top: a.top, bottom: a.bottom, listTop: b.top, listBottom: b.bottom };
    });
    const results = [];
    for (const adaptive of [false, true]) {
      await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready === true);
      await page.evaluate(adaptive => window.demo.navigation_mount(adaptive), adaptive); await settle();
      const list = page.getByRole('list', { name: 'Virtual entries' });
      check(await list.getAttribute('tabindex') === '0', 'list tab stop');
      await page.locator('.outside-focus').focus(); await page.keyboard.press('Tab'); await settle();
      check(await list.evaluate(element => document.activeElement === element), 'Tab did not enter the collection');
      await page.keyboard.press('ArrowDown'); await settle();
      check((await focused()).index === 0, 'root ArrowDown skips first visible row');
      // Key events arriving before the same commit must not overwrite each other.
      await page.evaluate(() => {
        const item = document.activeElement;
        for (let i = 0; i < 35; i++) item.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
      }); await settle();
      let state = await focused(); check(state.index === 35 && state.visible, `batch navigation ${JSON.stringify(state)}`);
      await page.keyboard.press('PageDown'); await settle();
      const down = await focused(); check(down.index > 35 && down.fullyVisible, `PageDown ${JSON.stringify(down)}`);
      await page.keyboard.press('PageUp'); await settle();
      check((await focused()).index < down.index && (await focused()).visible, 'PageUp did not move backwards');
      let maxRows = 0, maxSlots = 0, maxKeyReads = 0;
      for (let i = 0; i < 15; i++) {
        const before = await stats();
        await page.keyboard.press(i % 2 ? 'Home' : 'End'); await settle();
        const item = await focused(), current = await stats();
        check(item.index === (i % 2 ? 0 : 99_999_999), `Home/End ${JSON.stringify(item)}`);
        check(item.size === '100000000' && item.fullyVisible, `logical position/viewport ${JSON.stringify(item)}`);
        check(item.focusVisible && item.outline !== 'none', 'keyboard focus has no visible native outline');
        check(current.rows <= 17 && current.tasks === current.rows && current.pinned === 0, `bounded navigation ${JSON.stringify(current)}`);
        maxRows = Math.max(maxRows, current.rows); maxSlots = Math.max(maxSlots, current.slots); maxKeyReads = Math.max(maxKeyReads, current.keys - before.keys);
      }
      // Expose actual browser accessibility nodes, not just CSS/DOM attributes.
      const cdp = await context.newCDPSession(page);
      const ax = await cdp.send('Accessibility.getFullAXTree');
      const collection = ax.nodes.find(node => node.role?.value === 'list' && node.name?.value === 'Virtual entries');
      const items = ax.nodes.filter(node => node.role?.value === 'listitem' && !node.ignored);
      check(collection && items.length > 0 && items.length <= 17, 'browser AX tree missing bounded list/items');
      const axEvidence = { list: collection.name.value, items: items.length, properties: items[items.length - 1].properties };
      await cdp.detach();
      await page.keyboard.press('Home'); await settle();
      await page.keyboard.press('Tab'); await settle();
      check(await page.locator('.edit-0').evaluate(element => document.activeElement === element), 'Tab did not enter row editor');
      const editKeys = await page.evaluate(() => {
        const input = document.activeElement, result = {};
        for (const key of ['ArrowDown', 'Home', 'End', 'PageDown']) {
          const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }); input.dispatchEvent(event); result[key] = event.defaultPrevented;
        }
        return result;
      }); await settle();
      check(Object.values(editKeys).every(value => !value), 'list stole an editor key');
      await page.locator('.edit-0').fill('abcdef'); await settle();
      await page.keyboard.press('Home');
      const homeCaret = await page.locator('.edit-0').evaluate(input => input.selectionStart);
      await page.keyboard.press('End');
      const endCaret = await page.locator('.edit-0').evaluate(input => input.selectionStart);
      await page.keyboard.press('ArrowLeft');
      const leftCaret = await page.locator('.edit-0').evaluate(input => input.selectionStart);
      check(homeCaret === 0 && endCaret === 6 && leftCaret === 5, 'native editor caret navigation failed');
      for (const selector of ['.action-0', '.link-0']) {
        await page.locator(selector).focus();
        const prevented = await page.evaluate(() => {
          const event = new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true });
          document.activeElement.dispatchEvent(event); return event.defaultPrevented;
        });
        await settle(); check(!prevented, `list stole a control key: ${selector}`);
      }
      await page.locator('.edit-0').focus(); await settle();
      await page.keyboard.press('Shift+Tab'); await settle();
      check(await list.evaluate(element => document.activeElement === element), 'Shift+Tab failed to leave editor');
      await page.keyboard.press('Shift+Tab'); await settle();
      check(await page.locator('.outside-focus').evaluate(element => document.activeElement === element), 'list trapped Tab');
      // Removing focus returns to the collection, which remains navigable.
      await page.locator('.edit-1').focus(); await settle();
      await page.evaluate(() => window.demo.focus_jump(1_000_000_000)); await settle();
      await page.evaluate(() => window.demo.focus_remove()); await settle();
      check(await list.evaluate(element => document.activeElement === element), 'deleted focus did not return to list');
      await page.keyboard.press('Home'); await settle();
      check((await focused()).index === 0 && (await focused()).size === '99999999', 'navigation after deletion');
      await page.evaluate(() => window.demo.focus_prepend()); await settle();
      check((await focused()).index === 10_000 && (await focused()).size === '100009999', 'prepend ARIA position/total');
      const guards = await page.evaluate(() => {
        const item = document.activeElement;
        return [{ key: 'ArrowDown', ctrlKey: true }, { key: 'ArrowDown', shiftKey: true }, { key: 'ArrowDown', isComposing: true }].map(init => {
          const event = new KeyboardEvent('keydown', { ...init, bubbles: true, cancelable: true }); item.dispatchEvent(event); return event.defaultPrevented;
        });
      }); await settle(); check(guards.every(value => !value), 'modifier/composition navigation stolen');
      const final = await stats();
      await page.evaluate(() => window.demo.unmount()); await settle();
      const lifetime = await page.evaluate(() => JSON.parse(window.demo.focus_lifetimes()));
      check(lifetime.created === lifetime.dropped, 'navigation leaked row owners');
      results.push({ adaptive, down, maxRows, maxSlots, maxKeyReads, axEvidence, editKeys, editorSelection: { homeCaret, endCaret, leftCaret }, guards, final, lifetime });
    }
    check(errors.length === 0, errors.join('\n'));
    return { results, errors };
  } finally { await context.close(); }
}
