async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 900, height: 800 } });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', e => errors.push(e.message));
    const check = (value, message) => { if (!value) throw new Error(message); };
    const near = (a, b) => Number.isFinite(a) && Math.abs(a - b) <= 0.03;
    const settle = () => page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
    for (const hydrate of [false, true]) {
      await page.goto(hydrate ? 'http://127.0.0.1:8788/css.html' : 'http://127.0.0.1:8788/');
      await page.waitForFunction(() => window.ready === true);
      if (!hydrate) await page.addStyleTag({ content: await page.evaluate(() => window.demo.css_text()) });
      if (hydrate) await page.locator('.sheet-input').evaluate(e => {window.cssOriginal=e;e.value='pretyped🙂';e.focus();e.setSelectionRange(1,5,'backward');document.querySelector('.sheet-check input').checked=true;});
      await page.evaluate(value => window.demo.css_mount(value), hydrate); await settle();
      const frame = () => page.locator(hydrate ? '#css-ssr' : '#app');
      const read = () => frame().evaluate(root => {
        const box=root.querySelector('.sheet-box'), label=root.querySelector('.sheet-check'), input=root.querySelector('.sheet-input'), text=root.querySelector('.sheet-text');
        const rect=e=>[e.getBoundingClientRect().width,e.getBoundingClientRect().height];
        return {box:rect(box),check:rect(label),input:rect(input),color:getComputedStyle(text).color,value:input.value,sel:[input.selectionStart,input.selectionEnd,input.selectionDirection],same:!window.cssOriginal||input===window.cssOriginal};
      });
      await page.mouse.move(800,700); const initial=await read();
      check(near(initial.box[0],80) && near(initial.check[0],75) && near(initial.check[1],hydrate?40:20),`base class/descendant layout or adopted checkbox: ${JSON.stringify(initial)}`);
      check(initial.color==='rgb(18, 52, 86)','text inheritance');
      if (hydrate) check(initial.same && initial.value==='pretyped🙂' && JSON.stringify(initial.sel)==='[1,5,"backward"]' && near(initial.input[0],150),'CSS hydration damaged pre-input/focus');
      await frame().locator('.sheet-input').evaluate(e=>{window.cssOriginal=e;e.focus();e.setSelectionRange(1,5,'backward');});
      await page.evaluate(()=>window.demo.css_update(false,true,true)); await settle(); const disabled=await read();
      check(near(disabled.box[0],120) && near(disabled.check[0],100) && near(disabled.check[1],40),`specificity or semantic checked/disabled: ${JSON.stringify(disabled)}`);
      check(disabled.same && JSON.stringify(disabled.sel)==='[1,5,"backward"]','state patches replaced the editor');
      await page.evaluate(()=>window.demo.css_update(true,true,true)); await settle();
      await frame().locator('.sheet-check input').focus(); await settle(); const focused=await read();
      check(near(focused.check[0],75) && near(focused.check[1],60),'Check focus must target its semantic label');
      await frame().locator('.sheet-box').hover(); await settle(); check(near((await read()).box[1],28),'hover selector');
      const typography=() => frame().evaluate(root=>{
        const box=root.querySelector('.sheet-type'),words=root.querySelector('.sheet-words'),input=root.querySelector('.sheet-editor');
        const css=getComputedStyle(box),font=getComputedStyle(input);
        const rule=[...document.styleSheets].flatMap(s=>[...s.cssRules]).find(r=>r.selectorText?.includes('.sheet-type'));
        return {height:words.getBoundingClientRect().height,family:css.fontFamily,weight:css.fontWeight,space:css.whiteSpace,border:css.borderTopWidth,declaredBorder:rule?.style.borderTopWidth,dpr:window.devicePixelRatio,color:css.borderTopColor,radius:css.borderTopLeftRadius,opacity:css.opacity,inputWeight:font.fontWeight,value:input.value,sel:[input.selectionStart,input.selectionEnd,input.selectionDirection],same:!window.typeOriginal||input===window.typeOriginal,focused:document.activeElement===input};
      });
      await frame().locator('.sheet-editor').evaluate(e=>{window.typeOriginal=e;e.value='draft🙂 retained';e.focus();e.setSelectionRange(1,7,'backward');});
      const typeBefore=await typography();
      check(near(typeBefore.height,20) && typeBefore.weight==='450.5' && typeBefore.space==='nowrap' && typeBefore.family.includes('monospace'),'base typography');
      // Browsers quantize used border widths to device pixels under page zoom.
      // Keep the authored CSS exact; bound the used width by one physical pixel.
      const usedBorder=parseFloat(typeBefore.border);
      check(typeBefore.declaredBorder==='2px' && Number.isFinite(usedBorder) && usedBorder>0 && Math.abs(usedBorder-2)<=1/typeBefore.dpr+0.03 && typeBefore.color==='rgb(1, 2, 3)' && typeBefore.radius==='8px' && typeBefore.opacity==='0.5',`decorations: ${JSON.stringify(typeBefore)}`);
      await page.evaluate(()=>window.demo.css_type_wrap(true)); await settle(); const typeWrapped=await typography();
      check(typeWrapped.height>=60 && typeWrapped.weight==='700' && typeWrapped.inputWeight==='700' && typeWrapped.space==='pre-wrap','typed inherited typography must override the sheet');
      check(typeWrapped.same && typeWrapped.focused && typeWrapped.value==='draft🙂 retained' && JSON.stringify(typeWrapped.sel)==='[1,7,"backward"]','font reflow damaged editor state');
      await page.evaluate(()=>window.demo.css_type_wrap(false)); await settle(); const typeReset=await typography();
      check(near(typeReset.height,20) && typeReset.weight==='450.5' && typeReset.same && typeReset.focused,'removing inline typography must restore stylesheet values');
      await page.evaluate(()=>window.demo.unmount()); await settle(); check(await frame().locator('[data-gly-node]').count()===0,'CSS fixture leaked nodes');
      results.push({hydrate,initial,disabled,focused,typeBefore,typeWrapped,typeReset});
    }
    check(errors.length===0,errors.join('\n')); return {results,errors};
  } finally {await context.close();}
}
