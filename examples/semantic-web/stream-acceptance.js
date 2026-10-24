async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(); await page.bringToFront();
    const check = (value, message) => { if (!value) throw new Error(message); };
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error' && /Glory|Content Security Policy|Refused/.test(message.text())) errors.push(message.text()); });
    await page.addInitScript(() => {
      window.streamFrames = [];
      for (const type of ['glory:stream-patch', 'glory:stream-ready', 'glory:stream-error']) document.addEventListener(type, event => {
        window.streamFrames.push({ type, time: performance.now(), detail: event.detail, first: !!document.querySelector('.stream-first'), second: !!document.querySelector('.stream-second'), middle: !!document.querySelector('.stream-middle'), hidden: !!document.querySelector('.stream-hidden') });
      }, true);
    });
    const response = await page.goto('http://127.0.0.1:8789/page', { waitUntil: 'commit' });
    await page.waitForSelector('.stream-editor');
    const initial = await page.evaluate(() => {
      const editor = document.querySelector('.stream-editor'), row = document.querySelector('.stream-row-0');
      window.streamOriginal = { editor, row, check: document.querySelector('.stream-check input'), middle: document.querySelector('.stream-middle') };
      Element.prototype.moveBefore = undefined; DocumentFragment.prototype.moveBefore = undefined;
      editor.value = 'typed while streaming'; editor.setSelectionRange(2, 6); editor.dispatchEvent(new InputEvent('input', { bubbles: true, data: 'draft' }));
      window.streamOriginal.check.click(); window.streamOriginal.check.click();
      window.streamOriginal.middle.value = 'middle draft'; window.streamOriginal.middle.dispatchEvent(new Event('input', { bubbles: true }));
      row.value = 'row draft'; row.dispatchEvent(new Event('input', { bubbles: true })); row.focus(); row.setSelectionRange(1, 4);
      const list = document.querySelector('[role=list]'); list.scrollTop = 36;
      window.targetWrites = 0;
      const descriptor = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
      Object.defineProperty(HTMLInputElement.prototype, 'value', { ...descriptor, set(value) { if (this === editor || this === row) window.targetWrites++; descriptor.set.call(this, value); } });
      return { first: !!document.querySelector('.stream-first'), second: !!document.querySelector('.stream-second'), hidden: !!document.querySelector('.stream-hidden'), active: GloryStream.active(), rows: list.querySelectorAll('[role=listitem]').length, state: document.getElementById('stream-root').dataset.glyStreamState };
    });
    check(!initial.first && !initial.second && !initial.hidden && initial.active === 1, JSON.stringify(initial));
    await page.waitForSelector('.stream-first');
    const middle = await page.evaluate(() => ({ second: !!document.querySelector('.stream-second'), middle: !!document.querySelector('.stream-middle'), rowSame: document.querySelector('.stream-row-0') === window.streamOriginal.row, order: [...document.querySelectorAll('.stream-keyed input')].map(node => node.className), focused: document.activeElement === window.streamOriginal.row, selection: [window.streamOriginal.row.selectionStart, window.streamOriginal.row.selectionEnd] }));
    check(!middle.second && !middle.middle && middle.rowSame && middle.focused && middle.selection.join() === '1,4', JSON.stringify(middle));
    await page.waitForFunction(() => window.ready && document.getElementById('stream-root').dataset.glyStreamState === 'ready');
    const ended = await page.evaluate(() => ({ editor: window.streamOriginal.editor.value, check: window.streamOriginal.check.checked,
      sameRow: document.querySelector('.stream-row-0') === window.streamOriginal.row, middle: document.querySelector('.stream-middle') === window.streamOriginal.middle,
      middleDraft: window.streamOriginal.middle.value, hidden: !!document.querySelector('.stream-hidden'), active: GloryStream.active(), writes: window.targetWrites,
      focused: document.activeElement === window.streamOriginal.row, selection: [window.streamOriginal.row.selectionStart, window.streamOriginal.row.selectionEnd],
      formatVersion: window.demo.html_format_version(), firstMarker: document.getElementById('stream-root').firstChild.nodeValue, blocks: document.querySelectorAll('script[data-gly-stream]').length,
      resources: JSON.parse(document.getElementById('stream-data').textContent).records.length, injected: window.streamInjected === 1, frames: window.streamFrames }));
    check(ended.editor === 'typed while streaming' && ended.check && ended.sameRow && ended.middle && ended.middleDraft === 'middle draft' && ended.hidden, JSON.stringify(ended));
    check(ended.active === 0 && ended.writes === 0 && ended.focused && ended.selection.join() === '1,4' && ended.firstMarker === `gly:v:${ended.formatVersion}` && ended.blocks === 0 && ended.resources === 2 && !ended.injected, JSON.stringify(ended));
    const anchor = () => page.evaluate(() => { const list = document.querySelector('[role=list]'), top = list.getBoundingClientRect().top + list.clientTop; return [...list.querySelectorAll('[role=listitem]')].map(node => ({ text: node.textContent, top: node.getBoundingClientRect().top - top, bottom: node.getBoundingClientRect().bottom - top })).find(row => row.top <= 0.1 && row.bottom > 0.1); });
    const before = await anchor();
    await page.evaluate(() => window.demo.stream_hydrate());
    const adopted = await page.evaluate(() => ({ ...JSON.parse(window.demo.stream_stats()), dataGone:!document.getElementById('stream-data'), sameEditor: document.querySelector('.stream-editor') === window.streamOriginal.editor, sameRow: document.querySelector('.stream-row-0') === window.streamOriginal.row, rowDraft: document.querySelector('.stream-row-0').value, focused: document.activeElement === window.streamOriginal.row, selection: [window.streamOriginal.row.selectionStart, window.streamOriginal.row.selectionEnd], writes: window.targetWrites }));
    check(adopted.loads === 0 && adopted.dataGone && adopted.draft === 'typed while streaming' && adopted.rowDraft === 'row draft' && adopted.checked && adopted.sameEditor && adopted.sameRow && adopted.focused && adopted.selection.join() === '1,4' && adopted.writes === 0, JSON.stringify(adopted));
    const after = await anchor(); check(before.text === after.text && Math.abs(before.top - after.top) < 0.1, JSON.stringify({ before, after }));
    await page.locator('.stream-count').click(); await page.waitForFunction(() => JSON.parse(window.demo.stream_stats()).count === 1);
    let maxRows=0,maxNodes=0;
    for (let i=1;i<=20;i++) {
      await page.evaluate(offset=>{const list=document.querySelector('[role=list]');list.scrollTop=offset;list.dispatchEvent(new Event('scroll'));},((i*7919)%900)*1000);
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const state=await page.evaluate(()=>{const root=document.getElementById('stream-root'),all=[...root.querySelectorAll('[data-gly-key]')];return{rows:root.querySelectorAll('[role=listitem]').length,nodes:JSON.parse(window.demo.stream_stats()).nodes,unique:new Set(all.map(node=>node.getAttribute('data-gly-key')+'/'+node.getAttribute('data-gly-part'))).size===all.length,draft:document.querySelector('.stream-row-0').value,active:GloryStream.active()};});
      maxRows=Math.max(maxRows,state.rows);maxNodes=Math.max(maxNodes,state.nodes);
      check(state.rows<=15&&state.nodes<=60&&state.unique&&state.draft==='row draft'&&state.active===0,JSON.stringify(state));
    }
    const raw = await response.text();
    await page.evaluate(() => window.demo.unmount());
    check(await page.locator('[data-gly-node]').count() === 0, 'unmount clears adopted DOM');

    // A second real response finishes while the user is still composing.
    let releaseStart; const startGate=new Promise(resolve=>releaseStart=resolve); let assetCalls=0;
    await page.route('**/glory-stream-frame.js',async route=>{ if(++assetCalls===1) await startGate; await route.continue(); });
    await page.goto('http://127.0.0.1:8789/page', { waitUntil: 'commit' }); await page.waitForSelector('.stream-editor');
    const beforeStart=await page.evaluate(() => {
      const editor = document.querySelector('.stream-editor'); window.composingEditor = editor; editor.focus();
      editor.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      editor.value = 'composition draft'; editor.setSelectionRange(3, 7);
      editor.dispatchEvent(new InputEvent('input', { bubbles: true, isComposing: true, data: 'draft' }));
      return GloryStream.active();
    });
    check(beforeStart===0,'composition begins before controller startup'); releaseStart();
    await page.waitForFunction(() => window.ready && !!document.querySelector('.stream-second'));
    const composing = await page.evaluate(() => {
      let failure; try { window.demo.stream_hydrate(); } catch (error) { failure = String(error); }
      return { state: document.getElementById('stream-root').dataset.glyStreamState, value: window.composingEditor.value, data: !!document.getElementById('stream-data'), active: GloryStream.active(), failure };
    });
    check(composing.state === 'active' && composing.value === 'composition draft' && !composing.data && composing.active === 1 && composing.failure, JSON.stringify(composing));
    await page.evaluate(() => window.composingEditor.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true })));
    await page.waitForFunction(() => document.getElementById('stream-root').dataset.glyStreamState === 'ready');
    await page.evaluate(() => window.demo.stream_hydrate());
    const composed = await page.evaluate(() => ({ ...JSON.parse(window.demo.stream_stats()), focused: document.activeElement === window.composingEditor, value: window.composingEditor.value, selection: [window.composingEditor.selectionStart, window.composingEditor.selectionEnd], active: GloryStream.active() }));
    check(composed.loads === 0 && composed.value === 'composition draft' && composed.focused && composed.selection.join() === '3,7' && composed.active === 0, JSON.stringify(composed));
    await page.evaluate(() => window.demo.unmount());
    await page.unroute('**/glory-stream-frame.js');

    await page.goto('http://127.0.0.1:8789/reverse', { waitUntil: 'commit' });
    await page.waitForFunction(() => window.ready && document.getElementById('stream-root').dataset.glyStreamState === 'ready');
    const reversed = await page.evaluate(() => ({ frames: window.streamFrames, active: GloryStream.active() }));
    check(reversed.frames[0].second && !reversed.frames[0].first && reversed.active === 0, JSON.stringify(reversed));
    await page.evaluate(()=>{const editor=document.querySelector('.stream-editor');editor.focus();editor.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));editor.value='late composition';editor.dispatchEvent(new InputEvent('input',{bubbles:true,isComposing:true}));});
    await page.evaluate(() => window.demo.stream_hydrate());
    check(await page.evaluate(() => JSON.parse(window.demo.stream_stats()).loads) === 0, 'reversed completion still adopts final keys');
    await page.evaluate(()=>window.demo.stream_program('program after adoption'));
    const lateBefore=await page.locator('.stream-editor').inputValue(); check(lateBefore==='late composition','composition begun after End transfers to DOM host');
    await page.evaluate(()=>document.querySelector('.stream-editor').dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})));
    await page.waitForFunction(()=>document.querySelector('.stream-editor').value==='program after adoption'&&JSON.parse(window.demo.stream_stats()).draft==='program after adoption');
    const lateAfter=await page.locator('.stream-editor').inputValue();
    await page.evaluate(() => window.demo.unmount());

    // Validate malformed first operations against an independently parsed shell.
    const rejected = await page.evaluate(raw => {
      const source = new DOMParser().parseFromString(raw, 'text/html');
      const start = JSON.parse(source.querySelector('script[data-gly-stream]').textContent);
      const patch = [...source.querySelectorAll('script[data-gly-stream]')].map(node => JSON.parse(node.textContent)).find(frame => frame.kind === 'patch');
      const failures = [];
      for (const type of ['sequence', 'version', 'missing', 'tag', 'attribute']) {
        const root = document.importNode(source.getElementById('stream-root'), true); root.id = `test-${type}`; document.body.append(root);
        const controller = GloryStream.start(root, { ...start, data_id: `data-${type}` }); const before = root.innerHTML;
        const bad = JSON.parse(JSON.stringify(patch));
        if (type === 'sequence') bad.sequence = 999;
        if (type === 'version') bad.version = 999;
        if (type === 'missing') bad.ops = [{ op: 'suspend', key: '99999999' }];
        if (type === 'tag' || type === 'attribute') {
          bad.ops = [{ op: 'create', key: '9007199254740993', shape: { options: null, pieces: [{ part: 'main', parent: null, tag: type === 'tag' ? 'script' : 'div', attributes: type === 'attribute' ? { onclick: 'bad()' } : {}, properties: {} }], children: 'main', control: 'main', revision: 0 } }];
        }
        let failure; try { controller.apply(bad); } catch (error) { failure = String(error); }
        if (!failure || before !== root.innerHTML) throw new Error(`malformed ${type} mutated shell`);
        controller.cancel(); root.remove(); failures.push(type);
      }
      return failures;
    }, raw);
    const preciseKey = await page.evaluate(raw => {
      const source = new DOMParser().parseFromString(raw, 'text/html');
      const start = JSON.parse(source.querySelector('script[data-gly-stream]').textContent);
      const root = document.importNode(source.getElementById('stream-root'), true); root.id = 'test-precision'; document.body.append(root);
      const controller = GloryStream.start(root, { ...start, data_id: 'precision-data' });
      controller.apply({ version: start.version, kind: 'patch', sequence: 1, ops: [
        { op: 'create', key: '9007199254740993', shape: { options: null, pieces: [{part:'main',parent:null,tag:'span',attributes:{},properties:{}},{part:'content',parent:'main',text:'exact key'}],children:null,control:'main',revision:0 } },
        { op: 'place', key: '9007199254740993', parent: null, before: null },
      ] });
      const exact = root.querySelector('[data-gly-key="9007199254740993"]').textContent === 'exact key';
      root.remove(); return exact;
    }, raw);
    check(preciseKey, 'key survives above JS Number safe integer');
    await page.evaluate(() => new Promise(resolve => queueMicrotask(resolve)));
    check(await page.evaluate(() => GloryStream.active()) === 0, 'removing a root releases its controller');
    const isolated=await page.evaluate(raw=>{
      const source=new DOMParser().parseFromString(raw,'text/html'),blocks=[...source.querySelectorAll('script[data-gly-stream]')].map(node=>JSON.parse(node.textContent));
      const start=blocks[0],patch=blocks.find(frame=>frame.kind==='patch');
      const roots=['left','right'].map(name=>{const root=document.importNode(source.getElementById('stream-root'),true);root.id='isolate-'+name;document.body.append(root);return root;});
      const a=GloryStream.start(roots[0],{...start,data_id:'left-data'}),b=GloryStream.start(roots[1],{...start,data_id:'right-data'});
      const right=roots[1].querySelector('.stream-editor');right.value='right draft';right.focus();right.dispatchEvent(new Event('input',{bubbles:true}));
      a.apply(patch);
      const isolated=!!roots[0].querySelector('.stream-first')&&!roots[1].querySelector('.stream-first')&&right.value==='right draft'&&document.activeElement===right&&GloryStream.active()===2;
      a.cancel();b.cancel();roots.forEach(root=>root.remove());return isolated;
    },raw);
    check(isolated,'same numeric keys in two streaming roots stay isolated');
    await page.goto('http://127.0.0.1:8789/broken');await page.waitForFunction(()=>window.ready&&document.getElementById('stream-root').dataset.glyStreamState==='failed');
    const truncated=await page.evaluate(()=>({state:document.getElementById('stream-root').dataset.glyStreamState,active:GloryStream.active(),data:!!document.getElementById('stream-data'),error:window.streamFrames.find(frame=>frame.type==='glory:stream-error')?.detail,shell:!!document.querySelector('.stream-editor')}));
    check(truncated.state==='failed'&&truncated.active===0&&!truncated.data&&truncated.error&&truncated.shell,JSON.stringify(truncated));

    const cancelPage = await context.newPage(); await cancelPage.goto('http://127.0.0.1:8789/page', { waitUntil:'commit' }); await cancelPage.waitForSelector('.stream-editor');
    await cancelPage.evaluate(() => { GloryStream.cancel(document.getElementById('stream-root')); if (GloryStream.active()) throw new Error('controller not cancelled'); });
    await cancelPage.close();
    let cleanup;
    for (let i=0;i<150;i++) { cleanup=await (await context.request.get('http://127.0.0.1:8789/stats')).json(); if (!cleanup.active && !cleanup.timers) break; await new Promise(resolve=>setTimeout(resolve,20)); }
    check(cleanup.active === 0 && cleanup.timers === 0, JSON.stringify(cleanup));
    check(errors.length === 0, errors.join('\n'));
    return { initial, middle, ended, adopted, postAdoption:{maxRows,maxNodes}, anchor: { before, after }, beforeStart, composing, composed, reversed, lateComposition:{before:lateBefore,after:lateAfter}, rejected, preciseKey, isolated, truncated, cleanup, errors };
  } finally { await context.close(); }
}
