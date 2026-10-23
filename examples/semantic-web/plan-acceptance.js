async (hostPage) => {
  const page = await hostPage.context().newPage();
  const assert = (ok, message) => { if (!ok) throw new Error(message); };
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  const settle = () => page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
  try {
    await page.goto('http://127.0.0.1:8788/plans.html');
    await page.waitForFunction(() => window.ready);
    await page.evaluate(() => window.demo.plan_mount(false));
    assert(await page.locator('#app .plan-editor').count() === 2, 'plan instance count');
    assert(await page.locator('#app .plan-1 .plan-editor').inputValue() === 'other 1', 'literal revision');
    await page.locator('#app .plan-0 .plan-editor').evaluate(e => {
      window.planEditor = e; window.planToken = e.dataset.glyNode;
      e.focus(); e.setSelectionRange(1, 3, 'backward');
    });
    await page.evaluate(() => { window.demo.plan_edit('caption'); window.demo.plan_edit('check'); });
    assert(await page.evaluate(() => planEditor === document.activeElement && planEditor.selectionStart === 1 && planEditor.selectionEnd === 3), 'unrelated updates changed editor selection');
    await page.evaluate(() => window.demo.plan_edit('swap'));
    assert(await page.evaluate(() => planEditor === document.querySelector('#app .plan-0 .plan-editor') && planEditor.dataset.glyNode === planToken), 'keyed move replaced editor');
    await page.locator('#app .plan-0 .plan-editor').evaluate(e => {
      e.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      e.value = 'draft中'; e.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true }));
    });
    await settle();
    await page.evaluate(() => window.demo.plan_edit('value'));
    assert(await page.locator('#app .plan-0 .plan-editor').inputValue() === 'draft中', 'composition overwritten');
    await page.locator('#app .plan-0 .plan-editor').evaluate(e => e.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
    await settle();
    assert(await page.locator('#app .plan-0 .plan-editor').inputValue() === 'program🙂', 'composition pending value');
    await page.locator('#app .plan-0').evaluate(e => { e.setAttribute('poison', 'yes'); e.querySelector('input').setAttribute('poison', 'yes'); });
    await page.evaluate(() => window.demo.plan_edit('add'));
    assert(await page.locator('#app .plan-2[poison],#app .plan-2 [poison]').count() === 0, 'instance leaked into prototype');
    assert(await page.locator('#app .plan-2 .plan-editor').inputValue() === 'other 2', 'new row value contaminated');
    assert(await page.locator('#app .plan-fallback').count() === 1, 'Group fallback');
    await page.evaluate(() => window.demo.plan_edit('clear'));
    assert(await page.locator('#app .plan-row').count() === 0, 'plan cleanup');
    await page.reload(); await page.waitForFunction(() => window.ready);
    await page.locator('#plan-ssr .plan-0 .plan-editor').evaluate(e => {
      window.adoptEditor = e; window.adoptToken = e.dataset.glyNode;
      e.value = 'draft🙂'; e.focus(); e.setSelectionRange(1, 4, 'backward');
    });
    await page.evaluate(() => window.demo.plan_mount(true));
    assert(await page.evaluate(() => adoptEditor === document.querySelector('#plan-ssr .plan-0 .plan-editor') && adoptEditor.dataset.glyNode !== adoptToken && adoptEditor.value === 'draft🙂' && adoptEditor.selectionStart === 1 && adoptEditor.selectionEnd === 4), 'plan hydration changed identity/draft/selection');
    await page.evaluate(() => window.demo.plan_edit('add'));
    assert(await page.locator('#plan-ssr .plan-2 .plan-editor').inputValue() === 'other 2', 'post-adoption plan creation');
    assert(errors.length === 0, errors.join('\n'));
    return { instances: 3, keyedIdentity: true, selection: true, composition: true, isolatedPrototype: true, groupFallback: true, hydration: true };
  } finally { await page.close(); }
}
