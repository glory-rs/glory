async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const assert = { ok(value, message = 'expected true') { if (!value) throw new Error(message); },
      equal(a, b, message = `${a} !== ${b}`) { if (a !== b) throw new Error(message); },
      deepEqual(a, b) { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`${JSON.stringify(a)} !== ${JSON.stringify(b)}`); } };
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    await page.goto('http://127.0.0.1:8788/');
    await page.waitForFunction(() => window.ready === true);
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const stats = () => page.evaluate(() => JSON.parse(window.demo.stats()));
    const results = [];
    await settle();
    const initial = await stats();
    assert.equal(await page.locator('[role=listitem]').count(), 29);
    assert.ok(initial.keys < 40);
    await page.locator('[role=list]').evaluate(element => element.style.height = '240px');
    await page.waitForFunction(() => document.querySelectorAll('[role=listitem]').length <= 15);
    assert.equal(await page.locator('[role=listitem]').count(), 14);
    await page.locator('[role=list]').evaluate(element => element.style.height = '600px');
    await page.waitForFunction(() => document.querySelectorAll('[role=listitem]').length === 29);
    assert.equal(await page.locator('[role=list]').evaluate(element => getComputedStyle(element).overflowAnchor), 'none');
    results.push('ResizeObserver updates the visible window; list controls its own scroll anchor');
  
    await page.locator('.icon').click();
    await settle();
    assert.equal((await stats()).count, 1);
    assert.equal(await page.locator('.increment').innerText(), 'count 1icon');
    assert.equal(await page.locator('.icon').count(), 1);
    results.push('child activation once; text patch preserves icon');
  
    await page.locator('.editor').fill('abcde');
    await settle();
    await page.evaluate(() => { const input = document.querySelector('.editor'); window.originalEditor = input; input.focus(); input.setSelectionRange(2, 4); window.demo.increment(); });
    assert.deepEqual(await page.evaluate(() => { const input = document.querySelector('.editor'); return [input === window.originalEditor, document.activeElement === input, input.selectionStart, input.selectionEnd, input.value]; }), [true, true, 2, 4, 'abcde']);
    await page.evaluate(() => window.demo.set_value(''));
    assert.equal(await page.locator('.editor').inputValue(), '');
    results.push('typed empty value and caret retained on unrelated update');
  
    const immediateQuery = await page.evaluate(() => { window.demo.focus_editor(); const input = document.querySelector('.editor'); input.setSelectionRange(0, 0); window.demo.query_editor(true); return window.demo.query_result(); });
    assert.equal(immediateQuery, 'pending');
    await settle();
    assert.deepEqual(await page.evaluate(() => JSON.parse(window.demo.query_result())), { start: 0, end: 0 });
    await page.evaluate(() => window.demo.query_editor(false)); await settle();
    const bounds = await page.evaluate(() => JSON.parse(window.demo.query_result()));
    assert.ok(bounds.width > 0 && bounds.height > 0);
    results.push('ordered focus action and asynchronous real geometry/selection queries');
    await page.evaluate(() => {
      window.demo.set_value('a😀z');
      document.querySelector('.editor').setSelectionRange(1, 3);
      window.demo.query_editor(true);
      // Delivery follows a newer commit; the answer still belongs to its barrier.
      window.demo.set_value('after');
    });
    await settle();
    assert.deepEqual(await page.evaluate(() => JSON.parse(window.demo.query_result())), { start: 1, end: 3 });
    assert.equal(await page.locator('.editor').inputValue(), 'after');
    results.push('delayed query keeps its accepted commit and UTF-16 selection after a newer edit');
  
    await page.evaluate(() => { const input = document.querySelector('.editor'); input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true })); input.value = 'ni'; input.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true })); });
    await settle();
    await page.evaluate(() => window.demo.set_value('server'));
    assert.equal(await page.locator('.editor').inputValue(), 'ni');
    await page.evaluate(() => { const input = document.querySelector('.editor'); input.value = '你'; input.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '你' })); });
    await settle();
    assert.equal(await page.locator('.editor').inputValue(), 'server');
    assert.equal((await stats()).value, 'server');
    await page.locator('.editor').press('Enter'); await settle();
    assert.equal((await stats()).commits, 1);
    results.push('composition buffers program writes and rejects late old revision; commit');
  
    await page.locator('.check').evaluate(label => label.click()); await settle();
    assert.equal((await stats()).checked, true); assert.equal((await stats()).activations, 1);
    await page.evaluate(() => window.demo.set_checked(false));
    assert.equal(await page.locator('.check input').isChecked(), false);
    assert.equal(await page.locator('.check input').getAttribute('checked'), null);
    results.push('checkbox label activation once and false property reset');
  
    await page.evaluate(() => { const input = document.querySelector('[aria-label="key 0"]'); window.keyInput = input; input.value = 'draft'; input.focus(); input.setSelectionRange(1, 3); window.demo.reorder(); });
    assert.deepEqual(await page.evaluate(() => { const input = document.querySelector('[aria-label="key 0"]'); return [input === window.keyInput, document.activeElement === input, input.value, input.selectionStart, input.selectionEnd, [...document.querySelectorAll('.keyed input')].map(x => x.getAttribute('aria-label'))]; }), [true, true, 'draft', 1, 3, ['key 2', 'key 1', 'key 0']]);
    for (let i = 0; i < 20; i++) await page.evaluate(() => window.demo.toggle());
    assert.equal(await page.locator('.branch-a').count(), 1);
    assert.equal(await page.locator('.branch-b').count(), 1);
    results.push('keyed reorder preserves input identity/focus/selection; Group branch recycling');
    await page.evaluate(() => { const moveBefore = Element.prototype.moveBefore; Element.prototype.moveBefore = undefined; try { window.demo.reorder(); } finally { Element.prototype.moveBefore = moveBefore; } });
    assert.deepEqual(await page.locator('[aria-label="key 0"]').evaluate(input => [input === window.keyInput, document.activeElement === input, input.value, input.selectionStart, input.selectionEnd]), [true, true, 'draft', 1, 3]);
    results.push('insertBefore fallback restores focused control and selection');
  
    await page.locator('.range-editor').evaluate(input => { window.rangeEditor = input; input.value = 'kept'; });
    await page.evaluate(() => window.demo.show_range(false));
    assert.equal(await page.locator('.range-editor').count(), 0);
    await page.evaluate(() => window.demo.show_range(true));
    assert.equal(await page.locator('.range-editor').inputValue(), 'kept');
    assert.equal(await page.locator('.range-editor').evaluate(input => input === window.rangeEditor), true);
    results.push('Group suspend/resume moves its full range and preserves live control state');
  
    const beforePeer = await stats();
    for (const nested of [true, false]) {
      await page.evaluate(nested => window.demo.mount_peer(nested), nested);
      await page.locator('.peer-button').click(); await settle();
      assert.equal(await page.locator('.peer-button').innerText(), 'peer 1');
      assert.equal((await stats()).count, beforePeer.count);
    }
    assert.equal(await page.locator('style[data-gly-base]').count(), 1);
    results.push('nested and sibling Runtimes isolate event/graph state and share baseline CSS');
  
    let maxRows = 0, maxNodes = 0, maxSlots = 0, maxGraph = 0;
    for (let i = 0; i < 80; i++) {
      const offset = ((i * 7919) % 100_000) * 24_000;
      await page.evaluate(offset => window.demo.seek(offset), offset);
      await settle();
      const state = await stats();
      maxRows = Math.max(maxRows, await page.locator('[role=listitem]').count());
      maxNodes = Math.max(maxNodes, await page.locator('[data-gly-node]').count());
      maxSlots = Math.max(maxSlots, state.slots); maxGraph = Math.max(maxGraph, state.graph);
      // A logical seek may retain the same physical scrollTop in a new segment;
      // on_scroll reports native movement, so inspect row coordinates directly.
      const renderedOffset = await page.locator('[role=listitem]').first().evaluate(row => Number(row.innerText.slice(4)) * 24 + row.closest('[role=list]').scrollTop - parseFloat(row.style.top));
      assert.ok(Math.abs(renderedOffset - offset) < 2, `${renderedOffset} vs ${offset}`);
      assert.ok(state.handlers <= initial.handlers);
    }
    assert.ok(maxRows <= 34); assert.ok(maxNodes < 90); assert.ok(maxSlots < 160); assert.ok(maxGraph < 160);
    await page.evaluate(() => window.demo.seek(2_400_000_000)); await settle();
    assert.equal(await page.locator('[role=listitem]').last().innerText(), 'row 99999999');
    assert.ok(await page.locator('[role=list]').evaluate(element => element.scrollHeight <= 1_000_001));
    await page.evaluate(() => window.demo.seek(24_000_000)); await settle();
    const previous = await stats();
    const native = await page.locator('[role=list]').evaluate(element => { const before = element.scrollTop; for (let i = 0; i < 200; i++) { element.scrollTop += 1; element.dispatchEvent(new Event('scroll')); } return { before, after: element.scrollTop }; });
    await settle();
    const current = await stats();
    assert.ok(Math.abs(current.offset - (24_000_000 + native.after - native.before)) < 0.001);
    assert.ok(current.keys - previous.keys <= 34, 'one window key read for one host tick');
    const beforePixel = await stats();
    const nativePixel = await page.locator('[role=list]').evaluate(element => { const before = element.scrollTop; element.scrollTop += 1; element.dispatchEvent(new Event('scroll')); return element.scrollTop - before; }); await settle();
    const afterPixel = await stats();
    assert.ok(nativePixel > 0 && nativePixel < 1.1);
    assert.ok(Math.abs(afterPixel.offset - beforePixel.offset - nativePixel) < 0.001); assert.equal(afterPixel.keys, beforePixel.keys);
    results.push({ list: '100M lazy rows, 80 far seeks, last row, 200 scrolls coalesced, native pixel delta', maxRows, maxNodes, maxSlots, maxGraph, keyReadsPerTick: current.keys - previous.keys, nativePixel });
  
    await page.evaluate(() => window.demo.unmount()); await settle();
    assert.equal(await page.locator('#app').innerHTML(), '');
    assert.equal(await page.locator('#app').getAttribute('data-gly-app'), null);
    for (let i = 0; i < 3; i++) { await page.evaluate(() => { window.demo.mount('app'); window.demo.seek(1_000_000); window.demo.unmount(); }); await settle(); }
    assert.equal(await page.locator('[data-gly-node]').count(), 0);
    assert.deepEqual(errors, []);
    results.push('unmount clears nodes, observers, RAF and remounts without errors');
    return { results, errors, browser: await page.evaluate(() => navigator.userAgent) };
  } finally { await context.close(); }
}
