async hostPage => {
  const context = await hostPage.context().browser().newContext({viewport:{width:900,height:700}});
  try {
    const page = await context.newPage(), errors = [], results = [];
    page.on('pageerror', error => errors.push(error.message));
    for (const hydrate of [false,true]) {
      await page.goto('http://127.0.0.1:8788/owners.html');
      await page.waitForFunction(() => window.ready === true);
      const result = await page.evaluate(async hydrate => {
        const check = (ok, message) => {if(!ok) throw Error(message);};
        const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        const stats = () => JSON.parse(window.demo.owner_stats());
        const root = document.querySelector(hydrate ? '#owner-ssr' : '#app');
        const serverEdit = root.querySelector('.owner-edit');
        await window.demo.owner_mount(hydrate); await settle();
        if(hydrate) check(root.querySelector('.owner-edit') === serverEdit,'adoption replaced the owner button');
        const before = stats(), edit = root.querySelector('.owner-edit');
        for(let i=0;i<1000;i++) edit.click();
        await settle();
        const warm = stats();
        check(warm.count===1000 && warm.derivedValue===1000,'event values did not flush');
        check(warm.nodes===before.nodes+2 && warm.fields===1,'repeat selections accumulated aliases');
        check(warm.fieldAlive && warm.derivedAlive,'active event resources were retired early');
        const toggle = root.querySelector('.owner-toggle');
        toggle.click(); await settle();
        const hidden = stats();
        check(!hidden.fieldAlive && !hidden.derivedAlive && hidden.derivedValue===null,'event resources escaped the listener owner');
        check(hidden.nodes===before.nodes && hidden.fields===0 && hidden.targets===0,'hidden branch retained runtime paths');
        edit.click(); await settle();
        check(stats().count===1000 && stats().fields===0,'detached event mutated the model or registered a path');
        toggle.click(); await settle();
        let peakSlots=warm.slots, peakOwners=warm.owners;
        for(let cycle=0;cycle<100;cycle++) {
          root.querySelector('.owner-edit').click(); await settle();
          const live=stats();
          check(live.count===1001+cycle && live.derivedValue===live.count,'replacement listener used stale resources');
          check(live.nodes===warm.nodes && live.fields===1 && live.payload===before.payload,'replacement changed graph bound or payload');
          peakSlots=Math.max(peakSlots,live.slots);peakOwners=Math.max(peakOwners,live.owners);
          toggle.click(); await settle();
          const gone=stats();
          check(!gone.fieldAlive && !gone.derivedAlive && gone.nodes===before.nodes && gone.fields===0 && gone.targets===0,'branch retirement leaked');
          toggle.click(); await settle();
        }
        check(peakSlots<=warm.slots+4 && peakOwners<=warm.owners+4,'arena capacity followed remount history');
        const final=stats();
        check(final.count===1100 && final.payload===before.payload,'source model was lost across remounts');
        window.demo.unmount();
        check(window.demo.owner_stats()==='null' && root.childNodes.length===0,'owner app cleanup failed');
        return {hydrate,before,warm,hidden,final,peakSlots,peakOwners,cycles:100};
      },hydrate);
      results.push(result);
    }
    if(errors.length) throw Error(errors.join('\n'));
    return {results,errors};
  } finally {await context.close();}
}
