async (hostPage) => {
  const page = await hostPage.context().newPage();
  const assert = (ok, message) => { if (!ok) throw new Error(message); };
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  try {
    // The probe never owns exports or retired nodes: GC must see the real app's
    // references, including Nodes held indirectly in its private Plan Arrays.
    await page.addInitScript(() => {
      for (const name of ['instantiate', 'instantiateStreaming']) {
        const original = WebAssembly[name];
        WebAssembly[name] = function (...args) {
          return original.apply(this, args).then(value => {
            window.partsExports = new WeakRef((value.instance || value).exports);
            return value;
          });
        };
      }
    });
    await page.goto('http://127.0.0.1:8788/plans.html');
    await page.waitForFunction(() => window.ready);
    await page.evaluate(() => {
      window.demo.plan_parts_probe(0);
      const container = document.querySelector('.bank-victims');
      window.retiredParts = [...container.querySelectorAll('*')]
        .flatMap(element => [element, ...element.childNodes])
        .map(node => new WeakRef(node));
      window.retiredTokens = new Set([...container.querySelectorAll('[data-gly-node]')]
        .map(element => element.dataset.glyNode));
      window.originalSurvivor = document.querySelector('.bank-survivor');
      window.survivorToken = originalSurvivor.dataset.glyNode;
      // A late part lookup must still address the original Text, rather than
      // walking the changed live DOM and accidentally updating this prefix.
      originalSurvivor.prepend(document.createTextNode('host:'));
    });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('HeapProfiler.enable');
    await page.evaluate(() => window.demo.plan_parts_probe(1));
    let partial;
    const gcAttempts = [];
    for (let attempt = 0; attempt < 5; attempt++) {
      await cdp.send('HeapProfiler.collectGarbage');
      partial = await page.evaluate(() => {
      const exports = partsExports.deref();
      if (!exports) throw new Error('missing WASM exports');
      let livePlanArrays = 0, retiredReferences = 0;
      const retired = new Set(retiredParts.map(weak => weak.deref()).filter(Boolean));
      const inspect = node => {
        if (retired.has(node) || node instanceof Element && retiredTokens.has(node.dataset.glyNode)) retiredReferences++;
      };
      for (const table of Object.values(exports).filter(value => value instanceof WebAssembly.Table)) {
        for (let index = 0; index < table.length; index++) {
          const value = table.get(index);
          inspect(value);
          if (Array.isArray(value)) {
            if (value.some(node => node instanceof Element && node.classList.contains('bank-probe'))) livePlanArrays++;
            value.forEach(inspect);
          }
        }
      }
      return { livePlanArrays, retiredReferences, collected: retiredParts.filter(weak => !weak.deref()).length, total: retiredParts.length,
        retained: [...retired].map(node => ({name:node.nodeName, parent:node.parentElement?.className, connected:node.isConnected})) };
      });
      gcAttempts.push(partial);
      assert(partial.retiredReferences === 0, 'partially retired Nodes retained by WASM/Plan Array');
      if (partial.collected === partial.total) break;
      // WeakRef dereferences stay alive through the current JS job. End it
      // before the next collection; never relax the host-reference assertion.
      await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0)));
    }
    assert(partial.livePlanArrays === 1, 'must inspect the still-live Plan bank');
    assert(partial.retiredReferences === 0, 'partially retired Nodes retained by WASM/Plan Array');
    assert(partial.collected === partial.total, 'retired physical parts must be collectible: ' + JSON.stringify(partial));
    await page.evaluate(() => window.demo.plan_parts_probe(2));
    assert(await page.evaluate(() => {
      const buttons = [...document.querySelectorAll('.bank-survivor')];
      return buttons.length === 2 && buttons[0] === originalSurvivor &&
        buttons[0].dataset.glyNode === survivorToken && buttons[0].textContent === 'host:survived' &&
        buttons[1].textContent === 'fresh' && buttons[1].dataset.glyNode !== survivorToken;
    }), 'late lookup, survivor identity or fresh prototype isolation');
    await page.locator('.bank-survivor').first().click();
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert(await page.evaluate(() => window.demo.plan_parts_probe(3)) === 1, 'survivor listener lost');
    await page.locator('.bank-survivor').last().click();
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert(await page.evaluate(() => window.demo.plan_parts_probe(3)) === 1, 'prototype copied live listener');
    const memoryGrowth = await page.evaluate(() => {
      const original = Element.prototype.setAttribute;
      let grew = 0;
      Element.prototype.setAttribute = function (name, value) {
        if (this.classList.contains('bank-probe') && name === 'aria-level') {
          const memory = Object.values(partsExports.deref()).find(value => value instanceof WebAssembly.Memory);
          memory.grow(1);
          grew++;
        }
        return original.call(this, name, value);
      };
      try { window.demo.plan_parts_probe(5); }
      finally { Element.prototype.setAttribute = original; }
      return grew;
    });
    assert(memoryGrowth === 1, 'must grow WASM memory during the field batch');
    assert(await page.evaluate(() => {
      const node = document.querySelector('.bank-probe');
      window.goalRoot = node;
      window.goalRootToken = node.dataset.glyNode;
      return node.getAttribute('role') === 'heading' && node.getAttribute('aria-level') === '3' && node.getAttribute('aria-label') === '标题🙂中"\\';
    }), 'batched heading fields and Unicode name');
    await page.evaluate(() => window.demo.plan_parts_probe(6));
    assert(await page.evaluate(() => goalRoot === document.querySelector('.bank-probe') && goalRoot.dataset.glyNode === goalRootToken && goalRoot.getAttribute('role') === 'grid' && !goalRoot.hasAttribute('aria-level')), 'batched field removal and role update');
    await page.evaluate(() => window.demo.plan_parts_probe(7));
    assert(await page.evaluate(() => goalRoot.getAttribute('role') === 'group' && !goalRoot.hasAttribute('aria-level')), 'named View default role after Clear');
    const fault = await page.evaluate(() => {
      const original = Element.prototype.setAttribute;
      let injected = 0;
      Element.prototype.setAttribute = function (name, value) {
        if (this === goalRoot && name === 'role') { injected++; throw new Error('expected Plan field failure'); }
        return original.call(this, name, value);
      };
      try { window.demo.plan_parts_probe(8); }
      finally { Element.prototype.setAttribute = original; }
      return {injected, level:goalRoot.getAttribute('aria-level'), role:goalRoot.getAttribute('role'), survivor:originalSurvivor.textContent};
    });
    assert(fault.injected === 1 && fault.level === '2' && fault.role === 'group' && fault.survivor === 'host:prefix🙂', 'field failure must preserve the physical prefix, stop the suffix, and fault the app');
    await page.evaluate(() => { window.demo.plan_parts_probe(4); delete window.originalSurvivor; });
    await page.evaluate(() => { delete window.goalRoot; });
    await cdp.send('HeapProfiler.collectGarbage');
    const cleared = await page.evaluate(() => {
      let retainedParts = 0;
      for (const table of Object.values(partsExports.deref()).filter(value => value instanceof WebAssembly.Table)) {
        for (let index = 0; index < table.length; index++) {
          const value = table.get(index);
          const inspect = node => { if (node instanceof Element && node.hasAttribute('data-gly-node')) retainedParts++; };
          inspect(value);
          if (Array.isArray(value)) value.forEach(inspect);
        }
      }
      return {retainedParts, children: document.querySelector('#app').childNodes.length};
    });
    assert(cleared.retainedParts === 0 && cleared.children === 0, 'whole Plan teardown retained identities');
    assert(errors.length === 0, errors.join('\n'));
    await cdp.detach();
    return { partial, gcAttempts, memoryGrowth, fault, cleared, maxMembers: 256, errors };
  } finally { await page.close(); }
}
