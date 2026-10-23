async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 900, height: 800 } });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && message.text().includes('Glory')) errors.push(message.text()); });
    const assert = (value, message) => { if (!value) throw new Error(message); };
    const cdp = await context.newCDPSession(page);
    const ax = async () => (await cdp.send('Accessibility.getFullAXTree')).nodes.filter(n => !n.ignored).map(n => ({ role: n.role?.value, name: n.name?.value || '' }));
    const has = (tree, role, name) => tree.some(n => n.role === role && n.name === name);
    for (const hydrate of [false, true]) {
      await page.goto(hydrate ? 'http://127.0.0.1:8788/semantics.html' : 'http://127.0.0.1:8788/');
      await page.waitForFunction(() => window.ready === true);
      if (hydrate) await page.locator('.sem-input').evaluate(e => { e.value = 'pretyped🙂'; e.focus(); e.setSelectionRange(1, 4, 'backward'); window.savedSemanticInput = e; });
      await page.evaluate(value => window.demo.semantics_mount(value), hydrate);
      const input = page.locator('.sem-input'), checkbox = page.locator('.sem-check input');
      if (hydrate) assert(await input.evaluate(e => e === window.savedSemanticInput && e.value === 'pretyped🙂' && e === document.activeElement && e.selectionStart === 1 && e.selectionEnd === 4 && e.selectionDirection === 'backward'), 'metadata hydration replaced/reset pre-edit');
      const initial = await ax();
      for (const [role, name] of [['group','Panel'], ['group','Explicit group'], ['heading','Heading name'], ['button','Button name'], ['textbox','Editor name'], ['checkbox','Check name'], ['image','Image name'], ['link','Link name'], ['list','Collection'], ['listitem','Logical item'], ['group','Region 0'], ['main','Region 1'], ['navigation','Region 2'], ['region','Region 3'], ['status','Region 4'], ['alert','Region 5']]) assert(has(initial, role, name), `missing AX ${role}/${name} ${JSON.stringify(initial)}`);
      assert(await page.locator('.sem-decorative').getAttribute('aria-hidden') === 'true', 'decorative image not hidden');
      assert(await checkbox.getAttribute('aria-label') === 'Check name' && await page.locator('.sem-check').getAttribute('aria-label') === null, 'Check name belongs to control');
      const expected = hydrate ? 'pretyped🙂' : 'a🙂z';
      await input.evaluate(e => {
        window.semanticInput = e; window.semanticValueWrites = 0;
        const value = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
        Object.defineProperty(e, 'value', { configurable: true, get() { return value.get.call(this); }, set(next) { window.semanticValueWrites++; value.set.call(this, next); } });
        e.focus(); e.setSelectionRange(1, 4, 'backward'); e.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      });
      const held = async () => {
        const state = await input.evaluate(e => ({ same: e === window.semanticInput, value: e.value, focused: e === document.activeElement, start: e.selectionStart, end: e.selectionEnd, direction: e.selectionDirection, writes: window.semanticValueWrites }));
        assert(state.same && state.value === expected && state.focused && state.start === 1 && state.end === 4 && state.direction === 'backward' && state.writes === 0, JSON.stringify(state));
      };
      await page.evaluate(() => window.demo.semantics_stage(1)); await held();
      assert(await input.getAttribute('tabindex') === '-1', 'programmatic focus mapping');
      assert(await page.locator('.sem-item').getAttribute('aria-posinset') === '100000000' && await page.locator('.sem-item').getAttribute('aria-setsize') === '-1', 'logical/unknown item metadata');
      const updated = await ax();
      for (const [role, name] of [['status','Updated sem-panel'], ['group','Updated sem-group'], ['heading','Updated sem-heading'], ['button','Updated sem-button'], ['textbox','Updated sem-input'], ['checkbox','Updated sem-check'], ['image','Updated sem-image'], ['link','Updated sem-link']]) assert(has(updated, role, name), `missing updated AX ${role}/${name}`);
      await page.evaluate(() => { window.semanticMutations = []; window.semanticObserver = new MutationObserver(records => window.semanticMutations.push(...records)); window.semanticObserver.observe(document.querySelector('.sem-root'), { attributes: true, childList: true, characterData: true, subtree: true }); });
      await page.evaluate(() => window.demo.semantics_stage(2)); await held();
      assert(await page.evaluate(() => { const count = window.semanticMutations.length + window.semanticObserver.takeRecords().length; window.semanticObserver.disconnect(); return count; }) === 0, 'Keep emitted physical mutations');
      await input.evaluate(e => e.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
      await page.locator('.sem-button').focus(); await page.keyboard.press('Tab');
      assert(await checkbox.evaluate(e => e === document.activeElement), 'programmatic editor was still a tab stop');
      await input.evaluate(e => { e.focus(); e.setSelectionRange(1, 4, 'backward'); });
      await page.evaluate(() => window.demo.semantics_stage(3)); await held();
      assert(await input.getAttribute('tabindex') === null && await page.locator('.sem-panel').getAttribute('role') === null && await page.locator('.sem-panel').getAttribute('aria-label') === null, 'Clear retained default-group or tabindex metadata');
      assert(await page.locator('.sem-group').getAttribute('role') === 'group' && await page.locator('.sem-item').getAttribute('aria-posinset') === null && await page.locator('.sem-item').getAttribute('aria-setsize') === null, 'Clear lost explicit group role or retained item');
      const cleared = await ax();
      for (const [role, name] of [['heading','Visible heading'], ['button','Button label'], ['textbox',''], ['checkbox','Check label'], ['image','Image description'], ['link','Link label']]) assert(has(cleared, role, name), `name fallback ${role}/${name}`);
      await page.locator('.sem-button').focus(); await page.keyboard.press('Tab');
      assert(await input.evaluate(e => e === document.activeElement), 'Clear did not restore native tab policy');
      await input.evaluate(e => e.setSelectionRange(1, 4, 'backward'));
      const rejected = await page.evaluate(() => window.demo.semantics_stage(4));
      assert(rejected === 15, `rejection matrix ${rejected}`); await held();
      assert(has(await ax(), 'textbox', 'Recovered editor'), 'refusal poisoned later valid metadata');
      await page.keyboard.press('End'); await page.keyboard.type('!');
      assert((await input.inputValue()).endsWith('!'), 'editing did not recover');
      await page.evaluate(() => window.demo.unmount());
      assert(await page.locator(hydrate ? '#semantics-ssr [data-gly-kind]' : '#app [data-gly-kind]').count() === 0, 'metadata fixture retained nodes');
      const observed = tree => tree.filter(n => ['group','heading','button','textbox','checkbox','image','link','list','listitem','main','navigation','region','status','alert'].includes(n.role));
      results.push({ hydrate, actualAXRolesAndNames: true, ax: { initial: observed(initial), updated: observed(updated), cleared: observed(cleared) }, keepHasNoDOMWrites: true, clearRestoresNativeNamesAndTabPolicy: true, identitySelectionAndValuePreserved: true, rejected, cleanup: true });
    }
    await cdp.detach(); assert(errors.length === 0, JSON.stringify(errors));
    return { results, errors };
  } finally { await context.close(); }
}
