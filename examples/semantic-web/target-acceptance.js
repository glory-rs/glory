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
    await page.evaluate(() => window.demo.target_mount());
    assert(await page.locator('#app [data-gly-kind]').count() === 7, 'Group introduced a visible wrapper');
    const immediate = await page.evaluate(() => {
      window.demo.target_query();
      return JSON.parse(window.demo.target_stats());
    });
    assert(immediate.responses.length === 0 && immediate.queries === 32, 'query callbacks ran synchronously');
    await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).responses.length === 32);
    const stats = await page.evaluate(() => JSON.parse(window.demo.target_stats()));
    const expected = [];
    for (let node = 0; node < 8; node++) for (let kind = 0; kind < 4; kind++) {
      expected.push(`${node}:${kind}:${node !== 1 && (kind !== 2 || node === 4) ? 'ok' : 'unsupported'}`);
    }
    assert(JSON.stringify(stats.responses) === JSON.stringify(expected), JSON.stringify(stats.responses));
    assert(stats.queries === 0 && stats.nodes === 8, 'query/semantic node leak');
    await page.locator('.target-input').evaluate(e => {
      e.focus(); e.setSelectionRange(1, 3, 'backward'); window.targetInputIdentity = e;
      e.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    });
    const preflight = await page.evaluate(() => window.demo.target_preflight());
    assert(preflight === 6, 'preflight rejection matrix');
    assert(await page.locator('.target-view').evaluate(e => e.tagName === 'DIV' && e.getAttribute('role') === 'status'), 'legal same-tag patch failed');
    assert(await page.locator('.target-input').evaluate(e => e === window.targetInputIdentity && e === document.activeElement
      && e.selectionStart === 1 && e.selectionEnd === 3 && e.selectionDirection === 'backward'), 'preflight changed editing identity/focus/selection');
    await page.locator('.target-input').evaluate(e => e.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
    await page.locator('.target-button').click();
    await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).activations === 1);
    await page.locator('.target-input').fill('still usable🙂');
    assert(await page.locator('.target-input').inputValue() === 'still usable🙂', 'invalid registrations poisoned editing');
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#app [data-gly-kind]').count() === 0, 'unmount retained DOM');
    assert(errors.length === 0, JSON.stringify(errors));
    return { cases: 8, acceptedQueries: 32, groupHasNoWrapper: true, groupGeometryUnsupported: true,
      orderedAsyncResponses: true, preflight, editingIdentityPreserved: true, editingRecovered: true, cleanup: true, stats, errors };
  } finally { await context.close(); }
}
