async (hostPage) => {
  const context = await hostPage.context().browser().newContext();
  try {
    const page = await context.newPage();
    const results = [];
    for (const hydrate of [false, true]) {
      await page.goto(`http://127.0.0.1:8788/unicode-selection.html${hydrate ? '?hydrate=1' : ''}`);
      await page.getByRole('button', { name: 'Write Unicode', exact: true }).click();
      const root = hydrate ? '#input-ssr' : '#app';
      if (hydrate && !await page.locator('#mode').innerText().then(text => text.includes('preserved'))) {
        throw new Error('Hydration replaced the input');
      }
      for (const direction of ['forward', 'backward']) {
        await page.getByRole('button', { name: `Select ${direction}`, exact: true }).click();
        const selected = await page.locator(`${root} .input-slot-0,${root} .input-slot-1`).evaluateAll(fields =>
          fields.map(field => [field.value, field.selectionStart, field.selectionEnd, field.selectionDirection]));
        const expected = Array.from({ length: 2 }, () => ['A中😀Z', 1, 4, direction]);
        if (JSON.stringify(selected) !== JSON.stringify(expected)) throw new Error(JSON.stringify(selected));
      }
      await page.getByRole('button', { name: 'Reject half surrogate', exact: true }).click();
      if (!(await page.locator('#result').innerText()).startsWith('Rejected:')) throw new Error('Invalid boundary accepted');
      const retained = await page.locator(`${root} .input-slot-0`).evaluate(field =>
        [field.selectionStart, field.selectionEnd, field.selectionDirection]);
      if (JSON.stringify(retained) !== '[1,4,"backward"]') throw new Error('Rejection changed selection');
      await page.getByRole('button', { name: 'Select forward', exact: true }).click();
      results.push({ hydrate, textAndPassword: true, directions: 2, rejectedHalfSurrogate: true, recovered: true });
    }
    return results;
  } finally { await context.close(); }
}
