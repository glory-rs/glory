async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const assert = { ok(value, message = 'expected true') { if (!value) throw new Error(message); }, equal(a, b) { if (a !== b) throw new Error(`${a} !== ${b}`); }, same(a, b) { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`${JSON.stringify(a)} !== ${JSON.stringify(b)}`); } };
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const load = async () => { await page.goto('http://127.0.0.1:8788/ssr.html'); await page.waitForFunction(() => window.ready); };
    const scene = () => page.evaluate(() => {
      const root = document.querySelector('.h-list [role=list]'); const bounds = root.getBoundingClientRect(); const top = bounds.top + root.clientTop;
      const rows = [...root.querySelectorAll('[role=listitem]')].map(element => { const rect = element.getBoundingClientRect(); return { key: Number(element.textContent.slice(4)), top: rect.top - top, height: rect.height }; });
      let gap = 0; for (let i = 1; i < rows.length; i++) gap = Math.max(gap, Math.abs(rows[i].top - rows[i - 1].top - rows[i - 1].height));
      return { rows: rows.length, gap, anchor: rows.find(row => row.top <= 0.1 && row.top + row.height > 0.1), scrollHeight: root.scrollHeight };
    });
    await load();
    const server = await scene(); assert.ok(server.rows < 20 && server.gap < 0.1 && server.scrollHeight < 1000, JSON.stringify(server));
    assert.equal(await page.locator('.h-paragraph').textContent(), 'a\nb\ufffd<&>');
    assert.equal(await page.locator('.h-multiline').inputValue(), '\nline<&>\n末尾');
    await page.locator('.h-editor').fill('typed before wasm');
    await page.locator('.h-multiline').fill('draft\nmultiline');
    await page.locator('.h-check input').uncheck();
    await page.locator('[aria-label="hydrate key 0"]').fill('local draft');
    await page.locator('.h-list [role=list]').evaluate(root => root.scrollTop = 150);
    await page.locator('.h-editor').evaluate(input => { input.focus(); input.setSelectionRange(2, 6); });
    const before = await scene();
    const adoption = await page.evaluate(() => {
      const root = document.querySelector('#ssr'); const editor = document.querySelector('.h-editor');
      const originals = [...root.querySelectorAll('[data-gly-key]')].filter(element => !element.closest('[role=list]'));
      window.savedKeyInput = document.querySelector('[aria-label="hydrate key 0"]');
      let writes = 0; const property = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
      Object.defineProperty(editor, 'value', { configurable: true, get() { return property.get.call(this); }, set(value) { writes++; property.set.call(this, value); } });
      const observer = new MutationObserver(() => {}); observer.observe(root, { childList: true, subtree: true });
      window.demo.hydrate_fixture();
      const changes = observer.takeRecords(); observer.disconnect();
      const addedStatic = changes.filter(change => !change.target.parentElement?.closest('[role=list]') && !change.target.closest?.('[role=list]')).flatMap(change => [...change.addedNodes]).filter(node => node.nodeType === Node.ELEMENT_NODE);
      return { writes, same: originals.every(element => element.isConnected), addedStatic: addedStatic.length,
        focused: document.activeElement === editor, selection: [editor.selectionStart, editor.selectionEnd],
        editor: editor.value, multiline: document.querySelector('.h-multiline').value, checked: document.querySelector('.h-check input').checked,
        stats: JSON.parse(window.demo.hydrate_stats()) };
    });
    await settle(); const after = await scene();
    assert.equal(adoption.writes, 0); assert.equal(adoption.same, true); assert.equal(adoption.addedStatic, 0);
    assert.equal(adoption.focused, true); assert.same(adoption.selection, [2, 6]);
    assert.equal(adoption.editor, 'typed before wasm'); assert.equal(adoption.multiline, 'draft\nmultiline'); assert.equal(adoption.checked, false);
    assert.equal(adoption.stats.value, adoption.editor); assert.equal(adoption.stats.multiline, adoption.multiline); assert.equal(adoption.stats.checked, false);
    assert.equal(before.anchor.key, after.anchor.key); assert.ok(Math.abs(before.anchor.top - after.anchor.top) < 0.1, JSON.stringify({ before, after }));
    assert.ok(after.gap < 0.1 && after.scrollHeight <= 1_000_001);
    assert.equal(await page.evaluate(() => { try { window.demo.hydrate_fixture(); return false; } catch (error) { return String(error).includes('already claimed'); } }), true);
    await page.locator('.h-icon').click(); await settle();
    assert.equal(await page.locator('.h-add').innerText(), 'count 1icon');
    await page.evaluate(() => window.demo.hydrate_edit()); await settle();
    assert.equal(await page.locator('.h-empty').textContent(), 'now populated'); assert.equal(await page.locator('.h-branch').count(), 0);
    assert.same(await page.locator('[aria-label="hydrate key 0"]').evaluate(input => [input === window.savedKeyInput, input.value]), [true, 'local draft']);
    await page.evaluate(() => window.demo.hydrate_seek(24_000_000)); await settle(); await settle();
    assert.ok((await scene()).gap < 0.1);
    assert.equal(await page.evaluate(() => { const keys = [...document.querySelectorAll('#ssr [data-gly-key]')].map(element => element.getAttribute('data-gly-key') + ':' + element.getAttribute('data-gly-part')); return keys.length === new Set(keys).size; }), true);
    await page.evaluate(() => window.demo.unmount()); await settle();
    assert.equal(await page.locator('#ssr').innerHTML(), '');
  
    const mismatches = [];
    for (const mutation of ['version', 'class', 'marker', 'parser']) {
      await load();
      const result = await page.evaluate(mutation => {
        const root = document.querySelector('#ssr'); const editor = root.querySelector('.h-editor'); editor.value = 'keep this draft'; editor.focus();
        if (mutation === 'version') root.firstChild.nodeValue = 'gly:v:999';
        if (mutation === 'class') root.querySelector('.hydrated').className = 'wrong';
        if (mutation === 'marker') root.querySelector('.h-paragraph').firstChild.remove();
        if (mutation === 'parser') { const html = root.innerHTML; root.innerHTML = html.replace('<div ', '<p ').replace('</div><!--gly:/v:4-->', '</p><!--gly:/v:4-->'); }
        const before = root.innerHTML; const controls = [...root.querySelectorAll('input,textarea')]; let error = '';
        try { window.demo.hydrate_fixture(); } catch (caught) { error = String(caught); }
        return { error, preserved: before === root.innerHTML, controls: controls.every(element => element.isConnected), claimed: root.hasAttribute('data-gly-app') };
      }, mutation);
      assert.ok(result.error.includes(mutation === 'version' ? 'Version' : 'Mismatch'), JSON.stringify(result));
      assert.equal(result.preserved, true); assert.equal(result.controls, true); assert.equal(result.claimed, false); mismatches.push(mutation);
    }
    await load();
    assert.equal(await page.evaluate(() => {
      const element = document.querySelector('.hydrated'); const previous = element.className; element.className = 'wrong';
      try { window.demo.hydrate_fixture(); } catch (_) {} element.className = previous;
      window.demo.hydrate_fixture(); return document.querySelectorAll('.hydrated').length === 1;
    }), true);
    await page.evaluate(() => window.demo.unmount());
    await load();
    await page.evaluate(() => window.demo.hydrate_fixture()); await settle();
    assert.equal(await page.locator('.h-initial-focus').evaluate(input => document.activeElement === input), true);
    await page.evaluate(() => window.demo.unmount());
    assert.same(errors, []);
    return { server, before, after, adoption, mismatches, errors, browser: await page.evaluate(() => navigator.userAgent) };
  } finally { await context.close(); }
}
