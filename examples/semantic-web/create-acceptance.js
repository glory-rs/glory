// Verify the real DOM creation path, including contamination of an earlier
// clone, later creation in the same app, and native input reflection.
async hostPage => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1200, height: 800 } });
  try {
    await context.addInitScript(() => {
      const clone = Node.prototype.cloneNode, seen = new WeakSet();
      window.creationProbe = { prototypes: 0, clones: 0, violations: [] };
      Node.prototype.cloneNode = function (...args) {
        if (this instanceof Element && !this.isConnected && this.hasAttribute('data-gly-kind') &&
            !this.hasAttribute('data-gly-node') && !this.hasAttribute('data-gly-key')) {
          const probe = window.creationProbe;
          if (!seen.has(this)) { seen.add(this); probe.prototypes++; }
          probe.clones++;
          const allowed = new Set(['data-gly-kind', 'role', 'type', 'data-gly-enabled', 'data-gly-disabled', 'disabled']);
          if (this.childNodes.length || [...this.attributes].some(a => !allowed.has(a.name)) ||
              ('value' in this && this.value !== '') || ('checked' in this && this.checked)) {
            probe.violations.push(this.outerHTML);
          }
        }
        return clone.apply(this, args);
      };
    });
    const page = await context.newPage(), errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const assert = (value, message) => { if (!value) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await page.goto('http://127.0.0.1:8788/');
    await page.waitForFunction(() => window.ready === true);
    await settle();
    const bridge = await page.evaluate(async () => {
      const resource = performance.getEntriesByType('resource').find(entry =>
        entry.name.includes('/snippets/glory-web-') && entry.name.endsWith('/inline0.js'));
      if (!resource) throw Error('fresh-element bridge module was not loaded');
      const { gloryCloneElement } = await import(resource.name);
      const prototype = document.createElement('button');
      prototype.setAttribute('type', 'button');
      const identity = '18446744073709551615.4294967295.4294967295';
      let cases = 0;
      for (const [classes, style, live] of [
        [undefined, undefined, false], ['', '', true],
        ['名字🙂 "<&>\\\0', 'color:rgb(12, 34, 56);--label:"🙂"', false],
        ['row', 'invalid css; width:3.125px', true],
      ]) {
        const expected = prototype.cloneNode(false);
        if (classes != null) expected.setAttribute('class', classes);
        if (style != null) expected.setAttribute('style', style);
        expected.setAttribute('data-gly-node', identity);
        if (live) expected.setAttribute('data-gly-live', '');
        const actual = gloryCloneElement(prototype, classes, style, identity, live);
        if (actual.outerHTML !== expected.outerHTML || actual === prototype || actual.isConnected)
          throw Error('typed creation changed attribute values, order, identity or attachment');
        cases++;
      }
      const original = Element.prototype.setAttribute;
      let caught = false;
      try {
        Element.prototype.setAttribute = function (name, value) {
          if (name === 'style') throw new Error('creation failure sentinel');
          return original.call(this, name, value);
        };
        gloryCloneElement(prototype, 'row', 'width:1px', identity, true);
      } catch (error) { caught = error.message === 'creation failure sentinel'; }
      finally { Element.prototype.setAttribute = original; }
      if (!caught || prototype.outerHTML !== '<button type="button"></button>')
        throw Error('typed creation swallowed a host exception or mutated its prototype');
      return { cases, unicodeAndNul: true, absentVersusEmpty: true, hostException: true };
    });
    await page.evaluate(() => {
      const old = document.querySelector('.branch-a');
      window.poisonedBranch = old; window.oldBranchToken = old.getAttribute('data-gly-node');
      old.textContent = 'secret'; old.classList.add('poisoned'); old.setAttribute('data-poison', 'private');
      window.demo.toggle();
    });
    await settle();
    await page.evaluate(() => window.demo.toggle()); await settle();
    assert(await page.evaluate(() => {
      const fresh = document.querySelector('.branch-a');
      return fresh !== window.poisonedBranch && !window.poisonedBranch.isConnected &&
        fresh.textContent === 'first' && !fresh.classList.contains('poisoned') &&
        !fresh.hasAttribute('data-poison') && fresh.getAttribute('data-gly-node') !== window.oldBranchToken;
    }), 'new Text inherited content, class, attribute or identity from an earlier clone');
    await page.evaluate(() => window.demo.focus_mount(false)); await settle();
    await page.evaluate(() => {
      const old = document.querySelector('.edit-0'); window.poisonedInput = old;
      old.value = 'private draft'; old.type = 'password'; old.disabled = true; old.checked = true;
      old.setAttribute('data-poison', 'private'); old.setAttribute('aria-label', 'private');
      window.demo.focus_jump(280000);
    });
    await settle();
    assert(await page.evaluate(() => {
      const fields = [...document.querySelectorAll('#app [role=list] input')];
      const outside = document.querySelector('.outside-focus');
      return outside.value === 'outside' && !outside.disabled && fields.length > 0 && !window.poisonedInput.isConnected && fields.every(field => {
        const key = field.className.match(/edit-(\d+)/)?.[1];
        return key && field.value === `draft ${key}` && field.type === 'text' && !field.disabled &&
          !field.checked && !field.hasAttribute('data-poison') && !field.hasAttribute('aria-label');
      });
    }), 'new Input inherited a value, type, state, name or attribute from an earlier clone');
    await page.evaluate(() => window.demo.input_mount(false)); await settle();
    assert(await page.evaluate(() => {
      const fields = [...document.querySelectorAll('.input-form input,.input-form textarea')];
      return fields.length === 8 && fields.every(field => !field.disabled && !field.checked) &&
        fields[1].type === 'password' && fields[2].getAttribute('inputmode') === 'email' &&
        fields[7].value === '\nfirst\nsecond�🙂';
    }), 'fresh input purpose/state/value lowering changed');
    const probe = await page.evaluate(() => window.creationProbe);
    assert(probe.clones > 30 && probe.prototypes > 1 && !probe.violations.length, JSON.stringify(probe));
    await page.evaluate(() => window.demo.unmount());
    assert(await page.locator('#app [data-gly-node]').count() === 0, 'DOM cleanup retained identities');
    assert(!errors.length, JSON.stringify(errors));
    return { bridge, probe, freshBranchIdentity: true, isolatedTextAndInputClones: true, reflectedState: true, cleanup: true, errors };
  } finally { await context.close(); }
}
