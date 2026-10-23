async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1200, height: 800 } });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('console', m => { if (m.type() === 'error' && m.text().includes('Glory')) errors.push(m.text()); });
    const assert = (condition, message) => { if (!condition) throw new Error(message); };
    const equal = (a, b) => assert(JSON.stringify(a) === JSON.stringify(b), `${JSON.stringify(a)} !== ${JSON.stringify(b)}`);
    const ready = () => page.waitForFunction(() => window.ready === true);
    const stats = () => page.evaluate(() => JSON.parse(window.demo.input_stats()));
    const selection = selector => page.locator(selector).evaluate(e => [e.selectionStart, e.selectionEnd, e.selectionDirection]);
    await page.goto('http://127.0.0.1:8788/'); await ready();
    await page.evaluate(() => window.demo.input_mount(false));
    const initial = await stats();
    const canonical = [...Array(7).fill('  a�🙂b  '), '\nfirst\nsecond�🙂'];
    equal(initial.model.map(row => row[0]), canonical);
    equal(await page.locator('.input-form input,.input-form textarea').evaluateAll(fields => fields.map(e => e.value)), canonical);
    const modes = await page.locator('.input-form input').evaluateAll(fields => fields.map(e => [e.type, e.getAttribute('inputmode')]));
    equal(modes[2], ['text', 'email']); equal(modes[4], ['text', 'url']); equal(modes[6], ['text', 'decimal']);
    equal(await page.locator('.input-form input,.input-form textarea').evaluateAll(fields => fields.map(e => {
      e.setSelectionRange(0, 2, 'forward'); return [e.selectionStart, e.selectionEnd];
    })), Array.from({ length: 8 }, () => [0, 2]));
    assert(initial.raw[0].includes('\0') && initial.raw[7].startsWith('\r\n'), 'rendering rewrote business cages');
    await page.evaluate(() => { window.demo.input_write(0, 'manual', '100'); window.demo.input_write(0, 'next', 'auto'); });
    equal((await stats()).model[0], ['next', '101']);
    const conflict = await page.evaluate(() => {
      const result = [];
      for (const revision of ['100', '101']) {
        try { window.demo.input_write(0, 'conflict', revision); result.push('accepted'); }
        catch (error) { result.push(String(error).includes('InputRevision')); }
      }
      for (const commitEvent of [false, true]) {
        result.push(window.demo.input_event(0, '100', commitEvent));
        result.push(window.demo.input_event(0, '102', commitEvent));
      }
      return result;
    });
    equal(conflict, [true, true, 'stale', 'future-rejected', 'stale', 'future-rejected']);
    equal((await stats()).model[0], ['next', '101']);
    assert((await stats()).commits === 0, 'stale commit callback ran');
    await page.locator('.input-slot-0').fill('typed🙂');
    await page.waitForFunction(() => JSON.parse(window.demo.input_stats()).model[0][1] === '102');
    await page.locator('.input-slot-0').press('Enter');
    await page.waitForFunction(() => JSON.parse(window.demo.input_stats()).commits === 1);
    await page.locator('.input-slot-0').evaluate(e => {
      e.value = 'A\0🙂B'; e.setSelectionRange(1, 4, 'backward');
      e.dispatchEvent(new InputEvent('input', { bubbles: true, data: '\0' }));
    });
    await page.waitForFunction(() => JSON.parse(window.demo.input_stats()).model[0][0] === 'A�🙂B');
    equal(await selection('.input-slot-0'), [1, 4, 'backward']);
    equal(await page.locator('.input-slot-0').inputValue(), 'A�🙂B');
    const rejected = await page.evaluate(() => {
      try { window.demo.input_write(0, 'bad\nline', 'auto'); return false; }
      catch (error) { return String(error).includes('single-line'); }
    });
    assert(rejected, 'single-line declaration silently removed line breaks');
    await page.evaluate(() => window.demo.input_write(0, 'recovered', 'auto'));
    equal((await stats()).model[0], ['recovered', '104']);
    await page.locator('.input-slot-7').evaluate(e => { e.focus(); e.setSelectionRange(1, 6, 'backward'); });
    await page.evaluate(() => window.demo.input_write(7, '\nfirst\nsecond�🙂', 'auto'));
    equal(await selection('.input-slot-7'), [1, 6, 'backward']);
    equal((await stats()).model[7], ['\nfirst\nsecond�🙂', '2']);
    await page.locator('.input-slot-7').evaluate(e => {
      e.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      e.value += '中'; e.setSelectionRange(2, 6, 'backward');
      e.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true }));
    });
    await page.waitForFunction(() => JSON.parse(window.demo.input_stats()).model[7][1] === '3');
    await page.evaluate(() => window.demo.input_write(7, '\r\nfirst\r\nsecond\0🙂中', 'auto'));
    equal((await stats()).model[7], ['\nfirst\nsecond�🙂中', '4']);
    equal(await selection('.input-slot-7'), [2, 6, 'backward']);
    await page.locator('.input-slot-7').evaluate(e => e.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '中' })));
    await page.waitForFunction(() => JSON.parse(window.demo.input_stats()).model[7][1] === '5');
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#app [data-gly-kind]').count() === 0, 'CSR retained DOM');

    await page.goto('http://127.0.0.1:8788/inputs.html'); await ready();
    equal(await page.locator('.input-form input,.input-form textarea').evaluateAll(fields => fields.map(e => e.value)), canonical);
    await page.evaluate(() => {
      const first = document.querySelector('.input-slot-0');
      first.focus(); first.value = 'dirty\0🙂'; first.setSelectionRange(5, 8, 'backward'); window.inputIdentity = first;
      const last = document.querySelector('.input-slot-7');
      last.value = '\r\nuser\r\nnote\0🙂'; last.setSelectionRange(2, 7, 'forward');
      last.setAttribute('data-gly-kind', 'wrong');
    });
    const failure = await page.evaluate(() => {
      try { window.demo.input_mount(true); return 'accepted'; }
      catch { return [document.querySelector('.input-slot-0').value, document.querySelectorAll('[data-gly-node]').length]; }
    });
    equal(failure, ['dirty\0🙂', 0]);
    equal(await selection('.input-slot-0'), [5, 8, 'backward']);
    await page.evaluate(() => { document.querySelector('.input-slot-7').setAttribute('data-gly-kind', 'input'); window.demo.input_mount(true); });
    const adopted = await stats();
    equal(adopted.model[0], ['dirty�🙂', '2']); equal(adopted.model[7], ['\nuser\nnote�🙂', '2']);
    equal(await selection('.input-slot-0'), [5, 8, 'backward']); equal(await selection('.input-slot-7'), [2, 7, 'forward']);
    assert(await page.locator('.input-slot-0').evaluate(e => e === window.inputIdentity && document.activeElement === e), 'adoption replaced editor/focus');
    equal(await page.locator('.input-slot-0').inputValue(), 'dirty�🙂');
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#input-ssr [data-gly-kind]').count() === 0, 'adopted DOM retained');
    equal(errors, []);
    return { purposes: 7, canonicalValues: true, mixedVersions: ['100', '101', '102'], staleAndFutureEditsAndCommits: true,
      nulSelection: true, compositionAliasSelection: true, invalidProgramValueRecovered: true, nativeSsr: true,
      failedAdoptionPreservedRawValue: true, adoptedSelectionAndIdentity: true, cleanup: true, errors };
  } finally { await context.close(); }
}
