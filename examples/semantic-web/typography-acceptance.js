async (hostPage) => {
  const context=await hostPage.context().browser().newContext({viewport:{width:900,height:800}});
  try {
    const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
    const check=(v,m)=>{if(!v)throw new Error(m);};
    await page.goto('http://127.0.0.1:8788/');await page.waitForFunction(()=>window.ready);
    await page.evaluate(()=>window.demo.adaptive_text_mount());
    const settle=()=>page.evaluate(()=>new Promise(r=>{let n=6;const tick=()=>--n?requestAnimationFrame(tick):r();requestAnimationFrame(tick);}));
    const layout=()=>page.evaluate(()=>{
      const root=document.querySelector('.adaptive [role=list]'),top=root.getBoundingClientRect().top+root.clientTop;
      const rows=[...root.querySelectorAll('[role=listitem]')].map(e=>{const r=e.getBoundingClientRect();return {key:Number(e.querySelector('.adaptive-content').textContent.match(/^row (\d+)/)[1]),id:e.getAttribute('data-gly-node'),top:r.top-top,height:r.height};});
      let gap=0;for(let i=1;i<rows.length;i++)gap=Math.max(gap,Math.abs(rows[i].top-rows[i-1].top-rows[i-1].height));
      return {rows,gap,anchor:rows.find(r=>r.top<=0.1&&r.top+r.height>0.1)};
    });
    const stats=()=>page.evaluate(()=>JSON.parse(window.demo.adaptive_stats()));
    await settle();await page.evaluate(()=>window.demo.adaptive_seek(20_000_007));await settle();
    const before=await layout();check(!!before.anchor,'missing visible anchor');
    const previous=before.rows.find(r=>r.key===before.anchor.key-1);check(!!previous,'missing overscan predecessor');
    await page.evaluate(key=>window.demo.adaptive_grow(key,1),previous.key);await settle();
    const after=await layout(),grown=after.rows.find(r=>r.key===previous.key);
    check(grown.height>=60 && grown.height>previous.height,'text did not wrap');
    check(before.anchor.id===after.anchor.id && before.anchor.key===after.anchor.key && Math.abs(before.anchor.top-after.anchor.top)<0.1,'text reflow lost the keyed viewport anchor');
    let maxRows=0,maxMeasured=0,maxSlots=0,maxGraphSlots=0;
    // 600px viewport / 20px estimate, four overscan rows per side, plus
    // partial-edge rounding. The older 24px fixture's 35-row bound is smaller.
    const rowBudget=Math.ceil(600/20)+2*4+2;
    for(let i=0;i<25;i++){
      await page.evaluate(offset=>window.demo.adaptive_seek(offset),i%2===0?1_900_000_007:i*7919*200);await settle();
      const s=await stats(),frame=await layout();maxRows=Math.max(maxRows,s.rows);maxMeasured=Math.max(maxMeasured,s.measured);maxSlots=Math.max(maxSlots,s.slots);maxGraphSlots=Math.max(maxGraphSlots,s.graphSlots);
      check(s.rows<=rowBudget && s.measured<=64 && s.heightCapacity<4096 && s.slots<=rowBudget*4 && s.graphSlots<300,`unbounded text rows: ${JSON.stringify(s)}`);
      check(frame.gap<0.1 && !!frame.anchor,'text list has visible gaps or overlaps');
    }
    await page.evaluate(()=>window.demo.unmount());await settle();check(await page.locator('[data-gly-node]').count()===0,'text list leaked');
    check(errors.length===0,errors.join('\n'));
    return {anchor:{key:before.anchor.key,idPreserved:before.anchor.id===after.anchor.id,topBefore:before.anchor.top,topAfter:after.anchor.top},heightBefore:previous.height,heightAfter:grown.height,maxRows,maxMeasured,maxSlots,maxGraphSlots,errors};
  } finally {await context.close();}
}
