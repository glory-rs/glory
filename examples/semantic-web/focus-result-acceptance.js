async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 800, height: 600 } });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('console', m => { if (m.type() === 'error' && m.text().includes('Glory')) errors.push(m.text()); });
    const assert = (value, message) => { if (!value) throw new Error(message); };
    const stats = () => page.evaluate(() => JSON.parse(window.demo.target_stats()));
    const focus = index => page.evaluate(i => window.demo.target_focus(i), index);
    const query = async index => {
      const immediate = await page.evaluate(i => {
        window.demo.target_focus_query(i); return JSON.parse(window.demo.target_stats());
      }, index);
      assert(immediate.queries === 1 && immediate.responses.length === 0, 'focus reply ran synchronously');
      await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).responses.length === 1);
      const result = await stats();
      assert(result.queries === 0, 'focus waiter retained');
      return result.responses[0];
    };
    const frame = () => page.evaluate(() => new Promise(requestAnimationFrame));
    await page.goto('http://127.0.0.1:8788/');
    await page.waitForFunction(() => window.ready === true);
    await page.evaluate(() => window.demo.target_mount());
    await focus(4);
    const matrix = [];
    for (let index = 0; index < 8; index++) matrix.push(await query(index));
    assert(JSON.stringify(matrix) === JSON.stringify(['false','unsupported','false','false','true','false','false','false']), JSON.stringify(matrix));
    await focus(2); // A normal span does not become focusable because focus() returned.
    assert(await query(2) === 'false' && await query(4) === 'true', 'non-focusable text invented focus');
    let decorativeRefused = false;
    try { await focus(6); } catch (error) { decorativeRefused = String(error).includes('focus on decorative image'); }
    assert(decorativeRefused, 'decorative image focus was not rejected before queuing');
    assert(await query(6) === 'false', 'image invented focus');
    await focus(5);
    assert(await query(5) === 'true' && await query(0) === 'false', 'Check must inspect its inner control; ancestors are not exact focus');
    const held = await page.evaluate(() => {
      window.demo.target_focus_query(5); window.demo.target_focus(4);
      return JSON.parse(window.demo.target_stats()).responses.length;
    });
    assert(held === 0, 'queued result delivered synchronously');
    await page.waitForFunction(() => JSON.parse(window.demo.target_stats()).responses.length === 1);
    assert((await stats()).responses[0] === 'true', 'later focus rewrote the accepted query snapshot');
    assert(await query(5) === 'false' && await query(4) === 'true', 'latest focus readback incorrect');
    await page.evaluate(() => {
      const root = document.querySelector('.target-view'); root.style.height = '40px'; root.style.overflow = 'auto'; root.scrollTop = 0;
    });
    await focus(3); await focus(4);
    assert(await page.locator('.target-view').evaluate(e => e.scrollTop === 0), 'Focus scrolled behind the list controller');
    const states = [];
    for (const kind of ['display', 'visibility', 'inert', 'opacity']) {
      await page.evaluate(kind => {
        const root = document.querySelector('.target-view');
        const editor = document.querySelector('.target-input');
        root.style.display = ''; root.style.visibility = ''; root.inert = false; editor.style.opacity = '';
        if (kind === 'display') root.style.display = 'none';
        if (kind === 'visibility') root.style.visibility = 'hidden';
        if (kind === 'inert') root.inert = true;
        if (kind === 'opacity') editor.style.opacity = '0';
      }, kind);
      await frame(); await focus(4);
      const result = await query(4);
      assert(result === (kind === 'opacity' ? 'true' : 'false'), `${kind}: ${result}`);
      states.push([kind, result]);
    }
    await page.evaluate(() => {
      const root = document.querySelector('.target-view'); root.style.visibility = 'hidden';
      const editor = document.querySelector('.target-input'); editor.style.opacity = ''; editor.style.visibility = 'visible';
    });
    await focus(4); assert(await query(4) === 'true', 'visible descendant cannot override inherited visibility');
    await page.evaluate(() => {
      const root = document.querySelector('.target-view'); root.style.display = 'none';
      document.querySelector('.target-input').style.display = 'block';
    });
    await frame(); await focus(4); assert(await query(4) === 'false', 'child display overrode display:none ancestor');
    await page.evaluate(() => {
      const root = document.querySelector('.target-view'); root.style.display = ''; root.style.visibility = '';
      document.querySelector('.target-input').style.visibility = '';
      window.demo.target_enabled(4, false);
    });
    const disabled = await page.evaluate(() => { try { window.demo.target_focus(4); return false; } catch (e) { return String(e).includes('disabled'); } });
    assert(disabled && await query(4) === 'false', 'disabled target focus mismatch');
    await page.evaluate(() => window.demo.target_enabled(4, true)); await focus(4);
    assert(await query(4) === 'true', 'focus did not recover');
    await page.evaluate(() => window.demo.target_focus_mode(0, true)); await focus(0);
    assert(await query(0) === 'true' && await query(4) === 'false', 'programmatic View focus failed');
    await page.evaluate(() => {
      const host = document.createElement('div'); host.id = 'focus-shadow'; document.body.append(host);
      host.attachShadow({mode:'open'}).append(document.getElementById('app'));
    });
    await focus(4);
    assert(await query(4) === 'true', 'focus query mistook the shadow host for the control');
    assert(await page.evaluate(() => document.activeElement.id === 'focus-shadow'
      && document.getElementById('focus-shadow').shadowRoot.activeElement.matches('.target-input')), 'shadow focus path not exercised');
    await page.evaluate(() => document.body.append(document.getElementById('focus-shadow').shadowRoot.getElementById('app')));
    await page.evaluate(() => {
      const editor = document.querySelector('.target-input'); window.detachedFocusEditor = editor;
      window.detachedFocusParent = editor.parentNode; editor.remove();
    });
    assert(await query(4) === 'unsupported', 'disconnected control query invented focus');
    await page.evaluate(() => window.detachedFocusParent.append(window.detachedFocusEditor));
    await focus(4); assert(await query(4) === 'true', 'reconnection did not recover');
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#app [data-gly-kind]').count() === 0, 'focus cleanup retained nodes');
    assert(errors.length === 0, JSON.stringify(errors));
    return { matrix, states, orderedAsyncFocusSnapshots: true, exactControlNotAncestor: true,
      preventScroll: true, shadowRootReadback: true, disabledAndDisconnectedRecovery: true, cleanup: true, errors };
  } finally { await context.close(); }
}
