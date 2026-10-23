async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1000, height: 900 } });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', error => errors.push(error.message));
    const check = (ok, message) => { if (!ok) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const stats = () => page.evaluate(() => JSON.parse(window.demo.fields_stats()));
    for (const hydrate of [false, true]) {
      await page.goto('http://127.0.0.1:8788/fields.html');
      await page.waitForFunction(() => window.ready === true);
      const root = hydrate ? '#fields-ssr' : '#app';
      if (hydrate) {
        await page.locator(`${root} .fields-editor`).fill('before🙂edit');
        await page.locator(`${root} .fields-editor`).evaluate(input => {
          input.focus(); input.setSelectionRange(1, 5, 'backward'); window.oldFieldInput = input;
        });
      }
      await page.evaluate(hydrate => window.demo.fields_mount(hydrate), hydrate); await settle();
      await page.locator(`${root} .fields-editor`).evaluate((input, hydrate) => {
        if (hydrate && input !== window.oldFieldInput) throw Error('hydration replaced input');
        if (!hydrate) { input.value = 'before🙂edit'; input.dispatchEvent(new InputEvent('input', { bubbles:true, data:null })); }
        input.focus(); input.setSelectionRange(1, 5, 'backward'); window.fieldInput = input;
        window.fieldValueWrites = 0;
        const property = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
        Object.defineProperty(input, 'value', {
          get() { return property.get.call(this); },
          set(value) { window.fieldValueWrites++; property.set.call(this, value); }, configurable:true
        });
      }, hydrate);
      await settle();
      const before = await stats();
      check(before.fields === 3 && before.slots === 3, 'field registry not canonical');
      const preserved = () => page.evaluate(() => {
        const input = window.fieldInput;
        return input.isConnected && input === document.activeElement && input.value === 'before🙂edit' &&
          input.selectionStart === 1 && input.selectionEnd === 5 && input.selectionDirection === 'backward' && window.fieldValueWrites === 0;
      });
      await page.evaluate(() => window.demo.fields_write('background', '', 1000)); await settle();
      const background = await stats();
      check(background.targetBuilds === before.targetBuilds + 1 && background.targetHits === before.targetHits + 999 && background.targetCaches === 1 && background.targetKeys === 2, 'burst rebuilt or retained too many targets');
      check(background.runs[0] === before.runs[0] && background.runs[1] === before.runs[1] && background.runs[2] === before.runs[2] + 1, 'unrelated field recomputed title/input');
      check(background.writes === before.writes + 1000 && background.payload === before.payload, 'source writes or model payload changed');
      check(await preserved(), 'background write changed input state');
      await page.evaluate(() => window.demo.fields_write('title', 'new <title>', 1)); await settle();
      const title = await stats();
      check(title.runs[0] === background.runs[0] + 1 && title.runs[1] === background.runs[1] && title.runs[2] === background.runs[2], 'title invalidated siblings');
      check(await page.locator(`${root} .fields-title`).textContent() === 'new <title>', 'title text not updated');
      check(await preserved(), 'title write changed input state');
      await page.evaluate(() => window.demo.fields_write('root', 'whole-model', 1)); await settle();
      const whole = await stats();
      check(whole.runs.every((value,index) => value === title.runs[index] + 1), 'whole-model fallback missed a field');
      check(await preserved(), 'whole-model echo reset input selection');
      await page.evaluate(() => window.demo.fields_write('draft', 'program🙂', 1)); await settle();
      const draft = await stats();
      check(draft.runs[0] === whole.runs[0] && draft.runs[1] === whole.runs[1] + 1 && draft.runs[2] === whole.runs[2], 'draft invalidated siblings');
      check(await page.locator(`${root} .fields-editor`).inputValue() === 'program🙂', 'controlled field value missing');
      await page.locator(`${root} .fields-editor`).press('End'); await page.keyboard.type('!'); await settle();
      check(await page.locator(`${root} .fields-editor`).inputValue() === 'program🙂!', 'field event write failed');
      check((await stats()).payload === before.payload, 'model payload moved');
      await page.evaluate(() => window.demo.unmount()); await settle();
      check(await page.locator(`${root} .fields-frame`).count() === 0, 'field DOM leaked');
      results.push({ hydrate, before, background, title, whole, draft, preserved:true, cleanup:true });
    }
    check(errors.length === 0, errors.join('\n'));
    return { results, errors };
  } finally { await context.close(); }
}
