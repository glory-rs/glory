async hostPage => {
  const context=await hostPage.context().browser().newContext({viewport:{width:900,height:700}});
  try {
    const page=await context.newPage(),errors=[],results=[];
    page.on('pageerror',error=>errors.push(error.message));
    for(const hydrate of [false,true]) {
      await page.goto('http://127.0.0.1:8788/queries.html');
      await page.waitForFunction(()=>window.ready===true);
      results.push(await page.evaluate(async hydrate=>{
        const check=(ok,message)=>{if(!ok)throw Error(message);};
        const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
        const stats=()=>JSON.parse(window.demo.query_stats());
        const empty=s=>s.queries===0&&s.queryOwners===0&&s.queryTargets===0;
        const root=document.querySelector(hydrate?'#query-ssr':'#app');
        const original=root.querySelector('.query-send');
        await window.demo.query_mount(hydrate);await settle();
        if(hydrate)check(root.querySelector('.query-send')===original,'SSR requester replaced');
        const target=root.querySelector('.query-target'),toggle=root.querySelector('.query-toggle');
        target.value='preserve external target';target.setSelectionRange(4,8);
        const before=stats();
        root.querySelector('.query-send').click();await settle();
        const warm=stats();
        check(warm.called===1&&warm.sent===1&&warm.dropped===1&&warm.derivedValue===1&&empty(warm),'live query did not complete');
        check(warm.nodes===before.nodes+2&&warm.fieldAlive&&warm.derivedAlive,'query resources not in the requester');
        toggle.click();await settle();
        const hidden=stats();
        check(!hidden.fieldAlive&&!hidden.derivedAlive&&hidden.fields===0&&hidden.nodes===before.nodes,'query resources outlived requester');
        toggle.click();await settle();
        let peakSlots=warm.slots,peakOwners=warm.owners;
        for(let cycle=0;cycle<100;cycle++) {
          // Both events enter one host input batch. The request is accepted,
          // then its requesting branch retires before the response is drained.
          root.querySelector('.query-send').click();toggle.click();await settle();
          const gone=stats();
          check(gone.sent===cycle+2&&gone.called===1&&gone.dropped===cycle+2,'late response invoked a retired requester');
          check(empty(gone)&&gone.fields===0&&gone.nodes===before.nodes,'retired query indices/resources accumulated');
          peakSlots=Math.max(peakSlots,gone.slots);peakOwners=Math.max(peakOwners,gone.owners);
          toggle.click();await settle();
          check(root.querySelector('.query-target')===target&&target.value==='preserve external target','external input lost identity or state');
          check(target.selectionStart===4&&target.selectionEnd===8,'external selection was changed');
        }
        root.querySelector('.query-send').click();await settle();
        const final=stats();
        check(final.called===2&&final.sent===102&&final.dropped===102&&final.count===2&&final.derivedValue===2&&empty(final),'query recovery failed');
        check(final.payload===before.payload&&peakSlots<=warm.slots+4&&peakOwners<=warm.owners+4,'source/capacity followed request history');
        window.demo.unmount();
        check(window.demo.query_stats()==='null'&&root.childNodes.length===0,'query app cleanup failed');
        return {hydrate,before,warm,hidden,final,peakSlots,peakOwners,cancelled:100};
      },hydrate));
    }
    if(errors.length)throw Error(errors.join('\n'));
    return {results,errors};
  } finally {await context.close();}
}
