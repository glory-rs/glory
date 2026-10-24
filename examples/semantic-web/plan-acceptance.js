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
    const verifyWide = root => page.evaluate(root => {
      const row = document.querySelector(`${root} .plan-wide`), nodes = [row, ...row.querySelectorAll('[data-gly-kind=text]')];
      return nodes.length === 18 && new Set(nodes.map(node => node.dataset.glyNode)).size === 18 && nodes.slice(1).every((node,index) => node.textContent === `wide ${index}`);
    }, root);
    assert(await verifyWide('#app'), 'large plan identity spill');
    const rememberFile = root => page.evaluate(root => {
      const button = document.querySelector(`${root} .plan-0 .plan-file`);
      const wrapper = button.parentElement, picker = wrapper.querySelector('input[type=file]');
      if (wrapper.localName !== 'span' || wrapper.style.display !== 'contents' || !picker || wrapper.firstElementChild !== button || wrapper.lastElementChild !== picker || wrapper.dataset.glyNode !== button.dataset.glyNode || picker.dataset.glyNode !== button.dataset.glyNode) throw new Error('plan File physical parts');
      window.planFile = { button, wrapper, picker, token: button.dataset.glyNode };
      const standalone = document.querySelector(`${root} .plan-file-root`);
      if (standalone.parentElement.localName !== 'span' || standalone.parentElement.style.display !== 'contents' || standalone.parentElement.dataset.glyNode !== standalone.dataset.glyNode || !standalone.parentElement.isConnected) throw new Error('plan File root placement');
    }, root);
    const verifyFile = root => page.evaluate(root => {
      const {button, wrapper, picker, token} = window.planFile;
      return button === document.querySelector(`${root} .plan-0 .plan-file`) && wrapper === button.parentElement && picker === wrapper.querySelector('input[type=file]') && picker.dataset.glyNode === token && button.dataset.glyNode === token;
    }, root);
    await rememberFile('#app');
    assert(await page.locator('#app .plan-1 .plan-editor').inputValue() === 'other 1', 'literal revision');
    await page.locator('#app .plan-0 .plan-editor').evaluate(e => {
      window.planEditor = e; window.planToken = e.dataset.glyNode;
      e.focus(); e.setSelectionRange(1, 3, 'backward');
    });
    await page.evaluate(() => { window.demo.plan_edit('caption'); window.demo.plan_edit('check'); });
    assert(await page.evaluate(() => planEditor === document.activeElement && planEditor.selectionStart === 1 && planEditor.selectionEnd === 3), 'unrelated updates changed editor selection');
    await page.evaluate(() => window.demo.plan_edit('swap'));
    assert(await page.evaluate(() => planEditor === document.querySelector('#app .plan-0 .plan-editor') && planEditor.dataset.glyNode === planToken), 'keyed move replaced editor');
    assert(await verifyFile('#app'), 'keyed move replaced File parts');
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
    assert(await page.evaluate(() => !planFile.button.isConnected && !planFile.wrapper.isConnected && !planFile.picker.isConnected), 'File plan cleanup');
    await page.reload(); await page.waitForFunction(() => window.ready);
    await page.locator('#plan-ssr .plan-0 .plan-editor').evaluate(e => {
      window.adoptEditor = e; window.adoptToken = e.dataset.glyNode;
      e.value = 'draft🙂'; e.focus(); e.setSelectionRange(1, 4, 'backward');
    });
    await page.evaluate(() => window.demo.plan_mount(true));
    assert(await verifyWide('#plan-ssr'), 'adopted large plan identities');
    await rememberFile('#plan-ssr');
    assert(await page.evaluate(() => adoptEditor === document.querySelector('#plan-ssr .plan-0 .plan-editor') && adoptEditor.dataset.glyNode !== adoptToken && adoptEditor.value === 'draft🙂' && adoptEditor.selectionStart === 1 && adoptEditor.selectionEnd === 4), 'plan hydration changed identity/draft/selection');
    await page.evaluate(() => window.demo.plan_edit('add'));
    assert(await page.locator('#plan-ssr .plan-2 .plan-editor').inputValue() === 'other 2', 'post-adoption plan creation');
    await page.evaluate(() => window.demo.plan_edit('swap'));
    assert(await verifyFile('#plan-ssr'), 'adopted File parts replaced on move');
    assert(errors.length === 0, errors.join('\n'));
    return { instances: 3, keyedIdentity: true, selection: true, composition: true, isolatedPrototype: true, groupFallback: true, fileParts: true, controlRoot: true, hydration: true };
  } finally { await page.close(); }
}
