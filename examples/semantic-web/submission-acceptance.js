async (hostPage) => {
  const page = await hostPage.context().newPage(), errors = [];
  const assert = (ok, message) => { if (!ok) throw Error(message); };
  page.on('pageerror', error => errors.push(error.message));
  try {
    await page.addInitScript(() => {
      window.submissionRuns = 0;
      for (const method of ['instantiate', 'instantiateStreaming']) {
        const original = WebAssembly[method];
        WebAssembly[method] = function (bytes, imports, ...rest) {
          for (const namespace of Object.values(imports || {})) {
            for (const key of Object.keys(namespace)) {
              if (key.toLowerCase().includes('submitplans')) {
                const run = namespace[key];
                namespace[key] = function (...args) { window.submissionRuns++; return run.apply(this, args); };
              }
            }
          }
          return original.call(this, bytes, imports, ...rest).then(result => {
            window.submissionExports = new WeakRef((result.instance || result).exports);
            return result;
          });
        };
      }
    });
    await page.goto('http://127.0.0.1:8788/plans.html');
    await page.bringToFront();
    await page.waitForFunction(() => window.ready);
    const initial = await page.evaluate(() => {
      demo.submission_probe(0);
      window.editor = document.querySelector('.submit-editor');
      editor.focus(); editor.value = 'native🙂draft'; editor.setSelectionRange(2, 5, 'backward');
      editor.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      window.submissionRuns = 0;
      demo.submission_probe(1);
      window.originalRows = [...document.querySelectorAll('.submit-row')];
      return { runs: submissionRuns, labels: originalRows.map(row => row.textContent), focused: document.activeElement === editor };
    });
    assert(initial.runs === 1 && initial.labels.join('|') === 'row 0press|row 1press|row 2press|row 3press', 'creation/initialization/attachment must share one run');
    assert(initial.focused, 'Plan attachment stole editor focus');
    const growth = await page.evaluate(() => {
      const descriptor = Object.getOwnPropertyDescriptor(Node.prototype, 'nodeValue');
      let grew = 0;
      Object.defineProperty(Node.prototype, 'nodeValue', { ...descriptor, set(value) {
        if (!grew && this.parentElement?.closest('.submit-row')) {
          const memory = Object.values(submissionExports.deref()).find(value => value instanceof WebAssembly.Memory);
          memory.grow(1); grew++;
        }
        return descriptor.set.call(this, value);
      }});
      submissionRuns = 0;
      try { demo.submission_probe(2); } finally { Object.defineProperty(Node.prototype, 'nodeValue', descriptor); }
      return { grew, runs: submissionRuns, preserved: originalRows.every((row, i) => document.querySelectorAll('.submit-row')[i] === row), labels: originalRows.map(row => row.textContent) };
    });
    assert(growth.grew === 1 && growth.runs === 1 && growth.preserved && growth.labels[3] === 'changed🙂 3press', 'growth detached the packet or changed identity');
    await page.evaluate(() => {
      window.retiredSubmissionText = new WeakRef(originalRows[0].querySelector('[data-gly-kind="text"]'));
      demo.submission_probe(5);
    });
    assert(await page.evaluate(() => document.querySelectorAll('.submit-row').length === 4 && originalRows[1].textContent === 'changed🙂 1press'), 'partial retirement damaged a sibling instance');
    const retired = await page.evaluate(() => {
      window.retiredSubmissionRows = originalRows.map(row => new WeakRef(row));
      delete window.originalRows;
      submissionRuns = 0;
      demo.submission_probe(3);
      return { runs: submissionRuns, rows: document.querySelectorAll('.submit-row').length, value: editor.value, start: editor.selectionStart, end: editor.selectionEnd, direction: editor.selectionDirection, focused: document.activeElement === editor };
    });
    // One partial original instance takes ordered fallback; complete instances
    // remain eligible for the host retirement segment.
    assert(retired.rows === 0 && retired.runs >= 1, 'full Plan retirement did not run');
    assert(retired.value === 'native🙂draft' && retired.start === 2 && retired.end === 5 && retired.direction === 'backward' && retired.focused, 'batch work rewrote editing state');
    // Blink's old layout fragments can retain detached rows until its next
    // rendering lifecycle. Check bank ownership after that lifecycle settles.
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const cdp = await page.context().newCDPSession(page);
    for (let i = 0; i < 4; i++) { await cdp.send('HeapProfiler.collectGarbage'); await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0))); }
    const released = await page.evaluate(() => ({ rows: retiredSubmissionRows.map(ref => !!ref.deref()), text: !!retiredSubmissionText.deref() }));
    assert(released.rows.every(alive => !alive) && !released.text, 'shared bank retained retired nodes: ' + JSON.stringify(released));
    await page.evaluate(() => { demo.submission_probe(9); demo.submission_probe(0); demo.submission_probe(1); });
    const fault = await page.evaluate(() => {
      const original = Element.prototype.setAttribute;
      let injected = 0;
      Element.prototype.setAttribute = function (name, value) {
        if (name === 'class' && value.includes('throw-here')) { injected++; throw Error('submission injected fault'); }
        return original.call(this, name, value);
      };
      let faulted;
      try { faulted = demo.submission_probe(4); } finally { Element.prototype.setAttribute = original; }
      return { injected, faulted, labels: [...document.querySelectorAll('.submit-row')].map(row => row.textContent) };
    });
    assert(fault.injected === 1 && fault.faulted === 1 && fault.labels.length === 4 && fault.labels[0] === 'accepted prefix🙂press' && fault.labels[2] === 'row 2press', 'host failure did not retain the successful prefix and stop the suffix');
    await page.evaluate(() => demo.submission_probe(9));
    assert(await page.evaluate(() => document.getElementById('app').childNodes.length === 0), 'faulted cleanup left physical nodes');
    const focusedRetirement = await page.evaluate(() => {
      demo.submission_probe(0); demo.submission_probe(1);
      document.querySelector('.submit-row button').focus();
      demo.submission_probe(3);
      const clean = document.querySelectorAll('.submit-row').length === 0 && document.querySelector('.submit-editor').value === 'a🙂b';
      demo.submission_probe(9);
      return clean;
    });
    assert(focusedRetirement, 'focused Plan retirement must use the native loss barrier');
    const segments = await page.evaluate(() => {
      demo.submission_probe(0);
      submissionRuns = 0;
      demo.submission_probe(6);
      const createdRuns = submissionRuns, rows = [...document.querySelectorAll('.submit-row')];
      const descendants = rows.map(row => [row, ...row.querySelectorAll('*'), ...[...row.querySelectorAll('*')].flatMap(element => [...element.childNodes].filter(node => node.nodeType === 3))]);
      submissionRuns = 0;
      demo.submission_probe(7);
      const current = [...document.querySelectorAll('.submit-row')];
      return { createdRuns, fieldRuns: submissionRuns, count: current.length,
        preserved: rows.every((row, i) => row === current[i] && row.textContent === `changed🙂 ${i}press` && descendants[i].every(node => node.isConnected)) };
    });
    assert(segments.count === 600 && segments.createdRuns > 1 && segments.fieldRuns > 1 && segments.preserved, 'segment buffers reused live banks or lost field-only targets');
    const segmentFault = await page.evaluate(() => {
      const original = Element.prototype.setAttribute;
      let injected = 0;
      Element.prototype.setAttribute = function(name, value) {
        if (name === 'class' && value.includes('throw-here')) { injected++; throw Error('segment injected fault'); }
        return original.call(this, name, value);
      };
      let faulted;
      try { faulted = demo.submission_probe(8); } finally { Element.prototype.setAttribute = original; }
      const rows = [...document.querySelectorAll('.submit-row')];
      return { injected, faulted, count: rows.length, prefix: rows.slice(0, 300).every(row => row.textContent === 'segment prefix🙂press'),
        untouched: rows[300].textContent === 'changed🙂 300press', appended: rows.at(-1).textContent };
    });
    assert(segmentFault.injected === 1 && segmentFault.faulted === 1 && segmentFault.count === 601 && segmentFault.prefix && segmentFault.untouched && segmentFault.appended === 'accepted new instance🙂press', 'cross-segment host failure lost accepted creations or executed the suffix');
    await page.evaluate(() => demo.submission_probe(9));
    assert(await page.evaluate(() => document.getElementById('app').childNodes.length === 0), 'cross-segment fault cleanup retained nodes');
    assert(errors.length === 0, errors.join('\n'));
    return { initial, growth, retired, fault, focusedRetirement, segments, segmentFault, errors };
  } finally { await page.close(); }
}
