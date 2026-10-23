async hostPage => {
  const context = await hostPage.context().browser().newContext({ viewport:{width:1000,height:900} });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', e => errors.push(e.message));
    const check = (ok,message) => { if (!ok) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const stats = () => page.evaluate(() => JSON.parse(window.demo.entries_stats()));
    for (const hydrate of [false,true]) {
      await page.goto('http://127.0.0.1:8788/entries.html'); await page.waitForFunction(() => window.ready === true);
      const root = hydrate ? '#entries-ssr' : '#app';
      if (hydrate) {
        await page.locator(`${root} .entry-input9`).fill('pre🙂draft');
        await page.locator(`${root} .entry-input9`).evaluate(input => {
          input.focus(); input.setSelectionRange(1,5,'backward'); window.beforeEntry9 = input;
        });
      }
      await page.evaluate(hydrate => window.demo.entries_mount(hydrate),hydrate); await settle();
      await page.locator(`${root} .entry-input9`).evaluate((input,hydrate) => {
        if (hydrate && input !== window.beforeEntry9) throw Error('adoption replaced entry editor');
        if (!hydrate) { input.value='pre🙂draft'; input.dispatchEvent(new InputEvent('input',{bubbles:true})); }
        input.focus(); input.setSelectionRange(1,5,'backward'); window.entry9=input;
        window.entry7=input.parentElement.querySelector('.entry-input7');
        window.entryWrites=0;
        const descriptor=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value');
        Object.defineProperty(input,'value',{get(){return descriptor.get.call(this);},set(v){window.entryWrites++;descriptor.set.call(this,v);},configurable:true});
      },hydrate); await settle();
      const preserved = () => page.evaluate(() => {
        const input=window.entry9;
        return input.isConnected && input===document.activeElement && input.value==='pre🙂draft' && input.selectionStart===1 && input.selectionEnd===5 && input.selectionDirection==='backward' && window.entryWrites===0;
      });
      const before=await stats(); check(before.fields===7 && before.keys===2 && before.buckets===2,'unexpected live paths');
      await page.evaluate(() => window.demo.entries_write('title','new <7>')); await settle();
      const title=await stats(); check(title.runs.every((n,i) => n===before.runs[i]+(i===0?1:0)),'entry field invalidated unrelated fields or keys');
      check(await page.locator(`${root} .entry-title7`).textContent()==='new <7>','keyed field output missing');
      check(await preserved(),'other key edit state changed');
      await page.evaluate(() => window.demo.entries_write('reserve','')); await settle();
      const reserve=await stats(); check(reserve.runs.every((n,i) => n===title.runs[i]+1),'root rehash did not validate live paths');
      check(reserve.payload9===before.payload9 && await preserved(),'rehash copied payload or changed editor');
      await page.evaluate(() => window.demo.entries_write('swap','')); await settle();
      check(await page.locator(`${root} .entry-position`).textContent()==='9','Vec item did not follow its position');
      check((await stats()).runs.every((n,i) => n===reserve.runs[i]) && await preserved(),'Vec position invalidated map entries');
      await page.evaluate(() => window.demo.entries_write('remove','')); await settle();
      const removed=await stats();
      check(removed.targetBuilds===reserve.targetBuilds+2 && removed.targetHits===reserve.targetHits && removed.targetCaches===2 && removed.targetKeys===6,'selected map/Vec targets did not remain separate and bounded');
      check(removed.runs.every((n,i)=>n===reserve.runs[i]+(i<2?1:0)),'key removal recomputed unrelated entries');
      check(await page.locator(`${root} .entry-title7`).textContent()==='missing','removed key not observed');
      check(await page.locator(`${root} .entry-input7`).isDisabled() && await page.locator(`${root} .entry-input7`).inputValue()==='','missing key did not disable/clear its control');
      check(await preserved(),'removing another key changed live editor');
      await page.evaluate(() => window.demo.entries_write('remove','')); await settle();
      const noop=await stats();check(JSON.stringify(noop)===JSON.stringify(removed),'missing-key removal published work');
      const rejected=await page.evaluate(() => { try {window.demo.entries_write('title','orphan');return false;}catch(e){return String(e).includes('MissingField');} });
      const failed=await stats();
      check(rejected && failed.writes===removed.writes && failed.targetBuilds===removed.targetBuilds && failed.targetHits===removed.targetHits && failed.targetKeys===removed.targetKeys,'missing write executed, published or replaced a valid target');
      await page.evaluate(() => window.demo.entries_write('insert','')); await settle();
      check(await page.locator(`${root} .entry-title7`).textContent()==='Key <7>','reinsert not observed');
      check(await page.locator(`${root} .entry-input7`).isEnabled() && await page.locator(`${root} .entry-input7`).inputValue()==='draft7🙂','reinsert did not recover field values');
      check(await page.evaluate(() => window.entry7.isConnected && window.entry9.parentElement.querySelector('.entry-input7')===window.entry7),'reinsert replaced semantic control');
      check(await preserved(),'reinserting another key changed live editor');
      check((await stats()).payload9===before.payload9,'unrelated payload moved');
      const inserted=await stats();check(inserted.runs.every((n,i)=>n===removed.runs[i]+(i<2?1:0)),'reinsert recomputed unrelated entries');
      for(const action of ['append','pop']){await page.evaluate(action=>window.demo.entries_write(action,''),action);await settle();}
      check((await stats()).positions===inserted.positions && await preserved(),'tail edit invalidated unchanged position or editor');
      await page.evaluate(()=>window.demo.entries_write('prepend',''));await settle();
      check(await page.locator(`${root} .entry-position`).textContent()==='13','insert did not shift the subscribed position');
      await page.evaluate(()=>window.demo.entries_write('remove_first',''));await settle();
      check(await page.locator(`${root} .entry-position`).textContent()==='9' && (await stats()).positions===inserted.positions+2 && await preserved(),'positional delete did not restore the first value/editor');
      await page.locator(`${root} .entry-input9`).press('End'); await page.keyboard.type('!'); await settle();
      check(await page.locator(`${root} .entry-input9`).inputValue()==='pre🙂draft!','keyed edit event failed');
      const final=await stats();
      check(final.fields===before.fields && final.keys===before.keys && final.buckets===before.buckets,'path registry grew');
      await page.evaluate(() => window.demo.unmount()); await settle();
      check(await page.locator(`${root} .entries-frame`).count()===0,'keyed DOM leaked');
      results.push({hydrate,before,title,reserve,removed,final,missingRejected:rejected,identityPreserved:true,cleanup:true});
    }
    check(errors.length===0,errors.join('\n'));return {results,errors};
  } finally { await context.close(); }
}
