async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1000, height: 900 }, deviceScaleFactor: 1 });
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', e => errors.push(e.message));
    const check = (ok, message) => { if (!ok) throw Error(message); };
    const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const verify = async (c, root) => {
      const actual = await page.locator(`${root} .flex-parent`).evaluate(parent => {
        const p = parent.getBoundingClientRect();
        return { width:p.width, height:p.height, children:[...parent.children].map(n => {
          const r=n.getBoundingClientRect(); return [r.x-p.x,r.y-p.y,r.width,r.height];
        }) };
      });
      check(actual.children.length===2,`${c.name}: the fixture must contain both flex items`);
      check(Math.abs(actual.width-300)<0.05 && Math.abs(actual.height-200)<0.05, `parent ${c.name}: ${JSON.stringify(actual)}`);
      const delta = Math.max(...actual.children.flatMap((r,i)=>r.map((v,j)=>Math.abs(v-c.expected[i][j]))));
      check(Number.isFinite(delta) && delta<0.05, `${c.name}: ${JSON.stringify({actual,expected:c.expected,delta})}`);
      return {name:c.name,delta};
    };
    for (const hydrate of [false,true]) {
      await page.goto('http://127.0.0.1:8788/flex.html'); await page.waitForFunction(()=>window.ready===true);
      const root=hydrate?'#flex-ssr':'#app';
      if (hydrate) {
        await page.locator(`${root} .flex-input`).fill('pre🙂input');
        await page.locator(`${root} .flex-input`).evaluate(input=>{
          input.focus();input.setSelectionRange(1,5,'backward');
          window.beforeFlexInput=input;window.beforeFlexParent=document.querySelector('#flex-ssr .flex-parent');
        });
      }
      await page.evaluate(hydrate=>window.demo.flex_mount(0,hydrate),hydrate); await settle();
      const cases=await page.evaluate(()=>JSON.parse(window.demo.flex_cases_json()));
      check(cases.length===46,'flex matrix incomplete');
      const adopted = await page.locator(`${root} .flex-input`).evaluate((input,hydrate)=>{
        if (!hydrate) {input.value='pre🙂input';input.focus();input.setSelectionRange(1,5,'backward');}
        window.flexInput=input;window.flexParent=input.parentElement.querySelector('.flex-parent');
        window.flexChildren=[...window.flexParent.children];
        return !hydrate || input===window.beforeFlexInput && window.flexParent===window.beforeFlexParent;
      },hydrate);
      check(adopted,'SSR adoption replaced the flex box or pre-input');
      const measured=[];
      for (const c of cases.filter(c=>!c.text)) {
        await page.evaluate(i=>window.demo.flex_case(i),c.index); await settle();
        measured.push(await verify(c,root));
        const preserved=await page.evaluate(()=>{
          const input=window.flexInput, parent=window.flexParent;
          return input.isConnected && input===document.activeElement && input.value==='pre🙂input' && input.selectionStart===1 && input.selectionEnd===5 && input.selectionDirection==='backward' && parent.isConnected && [...parent.children].every((n,i)=>n===window.flexChildren[i]);
        });
        check(preserved,`${c.name}: editor or node identity changed`);
      }
      await page.locator(`${root} .flex-input`).press('End'); await page.keyboard.type('!'); await settle();
      check((await page.locator(`${root} .flex-input`).inputValue()).endsWith('!'),'editing failed after flex changes');
      await page.evaluate(()=>window.demo.unmount()); await settle();
      check(await page.locator(`${root} .flex-frame`).count()===0,'unmount left flex nodes');
      results.push({hydrate,adopted,cases:measured.length,maxDelta:Math.max(...measured.map(r=>r.delta)),editorPreserved:true,cleanup:true});
    }
    await page.goto('http://127.0.0.1:8788/flex.html'); await page.waitForFunction(()=>window.ready===true);
    const cases=await page.evaluate(()=>JSON.parse(window.demo.flex_cases_json()));
    const text=[];
    for (const c of cases.filter(c=>c.text)) {
      await page.evaluate(i=>window.demo.flex_mount(i,false),c.index); await settle();
      text.push(await verify(c,'#app'));
    }
    await page.evaluate(()=>window.demo.unmount()); await settle();
    check(errors.length===0,errors.join('\n'));
    return {results,text,errors};
  } finally { await context.close(); }
}
