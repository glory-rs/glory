async host => {
  const page = await host.context().newPage(), errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const assert = (ok, message) => { if (!ok) throw Error(message); };
  try {
    await page.goto('http://127.0.0.1:8788/'); await page.waitForFunction(() => window.ready);
    const result = await page.evaluate(async () => {
      const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const model = () => JSON.parse(window.demo.input_stats()).model[0];
      const send = element => element.dispatchEvent(new InputEvent('input', { bubbles: true, data: 'x' }));
      window.demo.input_mount(false);
      let editor = document.querySelector('.input-slot-0'), token = editor.dataset.glyNode;
      const [runtime, slot, generation] = token.split('.');
      const expected = JSON.stringify(model());
      const bad = [`+${token}`, `0${token}`, `${token}.0`, ` ${token}`, `${runtime}.0${slot}.${generation}`,
        `${runtime}.${slot}.0${generation}`, `18446744073709551616.${slot}.${generation}`,
        `${runtime}.4294967296.${generation}`, `${runtime}.${slot}.4294967296`, `${runtime}..${generation}`];
      for (const value of bad) {
        editor.dataset.glyNode = value; editor.value = 'forged'; send(editor); await settle();
        if (JSON.stringify(model()) !== expected) throw Error('aliased/overflow token reached business code: '+value);
      }
      editor.dataset.glyNode = token; editor.value = 'accepted'; send(editor); await settle();
      if (model()[0] !== 'accepted') throw Error('canonical identity no longer routes editing');
      window.demo.unmount(); window.demo.input_mount(false);
      editor = document.querySelector('.input-slot-0'); const next = editor.dataset.glyNode, fresh = JSON.stringify(model());
      if (next === token) throw Error('new app reused its runtime');
      editor.dataset.glyNode = token; editor.value = 'retired'; send(editor); await settle();
      if (JSON.stringify(model()) !== fresh) throw Error('retired runtime reached a live slot');
      editor.dataset.glyNode = next; editor.value = 'recovered'; send(editor); await settle();
      if (model()[0] !== 'recovered') throw Error('editing did not recover');
      window.demo.unmount();
      return { rejectedAliases: bad.length, retiredRuntime: true, canonicalEdit: true, recovered: true };
    });
    assert(errors.length === 0, errors.join('\n'));
    return result;
  } finally { await page.close(); }
}
