// Real browser keys exercise the WASM interpreter; synthetic IME protocol is explicit.
// Runner supplies Playwright page and an initialized module at window.keyboardApi.
export async function keyboardAcceptance(page) {
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  await page.evaluate(() => window.keyboardApi.keyboard_mount());
  const draft = page.getByRole('textbox', {name:'Dialog draft', exact:true});
  const count = () => page.locator('.dismiss-count').textContent();
  await draft.fill('draft');
  await draft.press('Enter');
  assert(await draft.inputValue() === 'draft\n', 'ancestor must preserve default multiline Enter');
  assert(await count() === '0', 'Enter must not dismiss');
  await draft.evaluate(el => el.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true, data:'你'})));
  await draft.press('Escape');
  await page.waitForTimeout(100);
  assert(await count() === '0', 'active composition must retain Escape');
  await draft.evaluate(el => el.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true, data:''})));
  await draft.evaluate(el => el.dispatchEvent(new KeyboardEvent('keydown', {bubbles:true, key:'Escape', isComposing:true})));
  await draft.evaluate(el => el.dispatchEvent(new KeyboardEvent('keydown', {bubbles:true, key:'Escape', repeat:true})));
  await draft.press('Alt+Escape');
  await page.waitForTimeout(100);
  assert(await count() === '0', 'IME, repeat and Alt keys must remain filtered');
  await draft.press('Escape');
  await page.waitForFunction(() => document.querySelector('.dismiss-count')?.textContent === '1');
  assert(await draft.inputValue() === 'draft\n', 'dismiss must not edit the draft');
  const own = page.getByRole('textbox', {name:'Explicit submit', exact:true});
  await own.fill('send');
  await own.press('Enter');
  await page.waitForFunction(() => document.querySelector('.submit-count')?.textContent === '1');
  assert(await own.inputValue() === 'send', 'explicit Submit should prevent newline');
  await own.press('Shift+Enter');
  assert(await own.inputValue() === 'send\n', 'Shift+Enter retains newline with own interest');
  await own.press('Escape');
  await page.waitForTimeout(100);
  assert(await count() === '1', 'nearest interested editor owns its Escape');
  await page.evaluate(() => window.keyboardApi.unmount());
  assert(await page.locator('#app').textContent() === '', 'unmount releases fixture');
  return {ancestorDismiss:1, explicitSubmit:1, compositionFiltered:true, draftPreserved:true};
}
