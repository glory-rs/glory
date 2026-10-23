async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 900, height: 800 } });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', error => errors.push(error.message));
    const assert = (value, message) => { if (!value) throw new Error(message); };
    const settle = () => page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
    for (const hydrate of [false, true]) {
      await page.goto(hydrate ? 'http://127.0.0.1:8788/sizes.html' : 'http://127.0.0.1:8788/');
      await page.waitForFunction(() => window.ready === true);
      if (!hydrate) await page.addStyleTag({ content: await page.evaluate(() => window.demo.size_css()) });
      if (hydrate) await page.locator('.size-input').evaluate(e => { e.value = 'pretyped🙂'; e.focus(); e.setSelectionRange(1, 5, 'backward'); window.sizeInput = e; window.sizeBox = document.querySelector('.size-box'); });
      await page.evaluate(value => window.demo.size_mount(value), hydrate);
      await settle();
      await page.locator('.size-input').evaluate((e, hydrate) => {
        if (hydrate && (e !== window.sizeInput || document.querySelector('.size-box') !== window.sizeBox || e.value !== 'pretyped🙂' || e.selectionStart !== 1 || e.selectionEnd !== 5 || e.selectionDirection !== 'backward')) throw new Error('size hydration lost dirty identity or selection');
        window.sizeInput = e; window.sizeBox = document.querySelector('.size-box');
        e.focus(); e.setSelectionRange(1, 5, 'backward');
      }, hydrate);
      const values = [];
      for (const [stage, width, height, max] of [[0,200,100,'none'],[1,60,30,'60px'],[2,100,60,'25%'],[3,140,70,'60px'],[4,80,40,''],[0,200,100,'none'],[5,80,40,'']]) {
        await page.evaluate(stage => window.demo.size_stage(stage), stage); await settle();
        const observed = await page.locator('.size-box').evaluate(e => {
          const r = e.getBoundingClientRect(), i = document.querySelector('.size-input');
          return { width: r.width, height: r.height, max: e.style.maxWidth, identity: e === window.sizeBox && i === window.sizeInput, value: i.value, selection: [i.selectionStart,i.selectionEnd,i.selectionDirection], focused: document.activeElement === i };
        });
        assert(Math.abs(observed.width-width)<0.04 && Math.abs(observed.height-height)<0.04 && observed.max === max, `size stage ${stage}: ${JSON.stringify(observed)}`);
        assert(observed.identity && observed.value === (hydrate ? 'pretyped🙂' : 'initial🙂') && observed.selection.join() === '1,5,backward' && observed.focused, `size patch reset editor ${stage}`);
        values.push({ stage, ...observed });
      }
      await page.evaluate(() => { window.sizeWrites=0; window.sizeObserver=new MutationObserver(records=>window.sizeWrites+=records.length); window.sizeObserver.observe(window.sizeBox,{attributes:true,attributeFilter:['style']}); });
      await page.evaluate(() => window.demo.size_stage(6)); await settle();
      assert(await page.evaluate(()=>window.sizeWrites) === 0, 'Keep wrote style');
      const rejected = await page.evaluate(()=>window.demo.size_stage(7)); await settle();
      assert(rejected === 6 && await page.evaluate(()=>window.sizeWrites) === 0, 'invalid limits changed physical style');
      await page.evaluate(()=>{window.sizeObserver.disconnect();window.demo.size_stage(0);}); await settle();
      assert(Math.abs((await page.locator('.size-box').boundingBox()).width-200)<0.04, 'rejection prevented recovery');
      await page.keyboard.press('End'); await page.keyboard.type('!');
      assert((await page.locator('.size-input').inputValue()).endsWith('!'), 'editor did not recover');
      await page.evaluate(()=>window.demo.unmount());
      assert(await page.locator(hydrate ? '#size-ssr [data-gly-kind]' : '#app [data-gly-kind]').count() === 0, 'size cleanup retained nodes');
      results.push({hydrate,values,rejected,keepHasNoStyleWrites:true,recovery:true,cleanup:true});
    }
    assert(errors.length === 0, JSON.stringify(errors));
    return {results,errors};
  } finally { await context.close(); }
}
