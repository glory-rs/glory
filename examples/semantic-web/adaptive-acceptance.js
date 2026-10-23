async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const assert = { ok(value, message = 'expected true') { if (!value) throw new Error(message); },
      equal(a, b) { if (a !== b) throw new Error(`${a} !== ${b}`); } };
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready);
    await page.evaluate(() => window.demo.adaptive_mount());
    const settle = async () => {
      let deadline;
      try { await Promise.race([page.evaluate(() => new Promise(resolve => { let remaining = 4; const frame = () => --remaining ? requestAnimationFrame(frame) : resolve(); requestAnimationFrame(frame); })), new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error('browser animation frames did not arrive within 5s')), 5000); })]); }
      finally { clearTimeout(deadline); }
    };
    const stats = () => page.evaluate(() => JSON.parse(window.demo.adaptive_stats()));
    const layout = () => page.evaluate(() => {
      const root = document.querySelector('.adaptive [role=list]'); const top = root.getBoundingClientRect().top + root.clientTop;
      const rows = [...root.querySelectorAll('[role=listitem]')].map(element => { const rect = element.getBoundingClientRect(); return { key: Number(element.innerText.slice(4)), top: rect.top - top, height: rect.height, id: element.getAttribute('data-gly-node') }; });
      let gap = 0; for (let i = 1; i < rows.length; i++) gap = Math.max(gap, Math.abs(rows[i].top - rows[i - 1].top - rows[i - 1].height));
      const anchor = rows.find(row => row.top <= 0.1 && row.top + row.height > 0.1);
      return { rows, gap, anchor, physical: root.scrollTop, viewport: root.clientHeight };
    });
    await settle(); let initial = await stats();
    assert.ok(initial.measured >= initial.rows && initial.measured <= 64);
    assert.ok((await layout()).gap < 0.1);
    const scrollbarViewport = await page.evaluate(() => {
      const root = document.querySelector('.adaptive [role=list]'); const before = root.clientHeight;
      const css = document.createElement('style'); css.id = 'scrollbar-fixture';
      css.textContent = '.adaptive [role=list]::-webkit-scrollbar{width:31px;height:31px}'; document.head.appendChild(css);
      root.style.overflowX = 'scroll'; return { before, after: root.clientHeight };
    }); await settle();
    assert.equal((await stats()).viewportHeight, scrollbarViewport.after);
    await page.evaluate(() => { document.getElementById('scrollbar-fixture').remove(); document.querySelector('.adaptive [role=list]').style.overflowX = ''; }); await settle();
    await page.evaluate(() => window.demo.adaptive_seek(4802)); await settle();
    const before = await layout(); const previous = before.rows[before.rows.findIndex(row => row.key === before.anchor.key) - 1];
    assert.ok(previous, 'overscan row before anchor');
    await page.evaluate(row => window.demo.adaptive_grow(row.key, row.height + 30.375), previous); await settle();
    const grown = await layout();
    assert.equal(grown.anchor.key, before.anchor.key); assert.equal(grown.anchor.id, before.anchor.id);
    assert.ok(Math.abs(grown.anchor.top - before.anchor.top) < 0.1, 'measurement keeps the viewport anchor');
    assert.ok(grown.gap < 0.1);
  
    await page.evaluate(() => window.demo.adaptive_prepend(1000)); await settle();
    const prepended = await layout();
    assert.equal(prepended.anchor.key, grown.anchor.key); assert.equal(prepended.anchor.id, grown.anchor.id);
    assert.ok(Math.abs(prepended.anchor.top - grown.anchor.top) < 0.1, 'large prepend keeps the anchor and input row identity');
    await page.evaluate(key => window.demo.adaptive_drop_before(key + 1), prepended.anchor.key); await settle();
    const deleted = await layout();
    assert.equal(deleted.anchor.key, prepended.anchor.key + 1);
    assert.ok(Math.abs(deleted.anchor.top) < 0.1); assert.ok(deleted.gap < 0.1);
  
    await page.evaluate(key => { const root = document.querySelector('.adaptive'); root.style.transformOrigin = 'top left'; root.style.transform = 'scale(1.25)'; window.demo.adaptive_grow(key, 44.375); }, deleted.anchor.key); await settle();
    assert.ok((await layout()).gap < 0.15, 'transforms do not inflate measured layout heights');
    await page.evaluate(() => document.querySelector('.adaptive').style.transform = ''); await settle();
    await page.evaluate(key => window.demo.adaptive_grow(key, 2_000_000.375), deleted.anchor.key); await settle();
    assert.ok(await page.locator('.adaptive [role=list]').evaluate(root => root.scrollHeight <= 1_000_001), 'a giant row cannot enlarge the bounded physical surface');
  
    let maxRows = 0, maxMeasured = 0, maxHeightSlots = 0, maxHeightCapacity = 0, maxNodeSlots = 0, maxGraphSlots = 0;
    const started = Date.now();
    for (let i = 1; i <= 80; i++) {
      if (Date.now() - started > 120000) throw new Error(`adaptive regression exceeded 120s at far seek ${i}`);
      await page.evaluate(offset => window.demo.adaptive_seek(offset), ((i * 7919) % 90_000) * 24_000); await settle();
      const state = await stats(); const view = await layout();
      maxRows = Math.max(maxRows, state.rows); maxMeasured = Math.max(maxMeasured, state.measured);
      maxHeightSlots = Math.max(maxHeightSlots, state.heightSlots); maxNodeSlots = Math.max(maxNodeSlots, state.slots); maxGraphSlots = Math.max(maxGraphSlots, state.graphSlots);
      maxHeightCapacity = Math.max(maxHeightCapacity, state.heightCapacity);
      assert.ok(state.measured <= Math.max(64, state.rows)); assert.ok(state.heightSlots <= Math.max(64, state.rows) * 29);
      assert.ok(state.heightCapacity <= 4096);
      assert.ok(state.rows <= 35 && state.slots <= 120 && state.handlers <= 37 && state.graphSlots < 300, JSON.stringify(state));
      assert.ok(view.anchor && view.gap < 0.1, `viewport covered without overlap/gaps: ${JSON.stringify(view)}`);
    }
    assert.equal(errors.length, 0);
    const result = { maxRows, maxMeasured, maxHeightSlots, maxHeightCapacity, maxNodeSlots, maxGraphSlots, initial, scrollbarViewport, anchorGrowth: { key: grown.anchor.key, yBefore: before.anchor.top, yAfter: grown.anchor.top }, prepend: { key: prepended.anchor.key, idPreserved: prepended.anchor.id === grown.anchor.id }, errors };
    await page.evaluate(() => window.demo.unmount()); await settle();
    assert.equal(await page.locator('[data-gly-node]').count(), 0);
    return result;
  } finally { await context.close(); }
}
