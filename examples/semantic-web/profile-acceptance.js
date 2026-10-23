async (hostPage) => {
  const context=await hostPage.context().browser().newContext({viewport:{width:1280,height:900},deviceScaleFactor:1});
  try {
    const page=await context.newPage();await page.bringToFront();
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    const check=(condition,message)=>{if(!condition)throw new Error(message)};
    await page.goto('http://127.0.0.1:8788/');await page.waitForFunction(()=>window.ready===true);
    const settle=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    await settle();await page.evaluate(()=>window.demo.profile_start());
    for(let i=1;i<=40;i++){
      const turn=await page.evaluate(i=>{window.demo.seek(i*19_123_000);window.demo.increment();return JSON.parse(window.demo.settle_probe(8));},i);
      check(turn.passes===1 && !turn.pending,'browser spun synchronous layout observation');await settle();
    }
    const measured=await page.evaluate(()=>JSON.parse(window.demo.profile_stats()));
    check(measured.enabled,'profile is not enabled');
    for(const phase of ['flush','validate','derive','structure','bind','build','window','reconcile','patch','commit','observe']) {
      check(measured.phases[phase].calls>0,`missing phase ${phase}`);
    }
    for(const [phase,sample] of Object.entries(measured.phases)) {
      check(sample.invalid_clock===0 && Number.isFinite(sample.elapsed_us) && sample.elapsed_us>=0,`invalid clock ${phase}`);
    }
    check(measured.phases.flush.elapsed_us>0,'Performance clock did not advance');
    for(const phase of ['layout','paint','draw','style','damage','construct','post_construct','box_layout','transform','paint_tree','layout_cleanup','hover','subdocuments','text_shape','text_break','text_align'])check(measured.phases[phase].calls===0,`invented browser internal phase ${phase}`);
    const budget=await page.evaluate(()=>JSON.parse(window.demo.stats()));
    check(budget.keys<2000,'profile introduced full-source reads');
    const burst=await page.evaluate(()=>JSON.parse(window.demo.dirty_burst(1000)));
    check(burst.source_writes>=1000 && burst.marked<=2*(burst.before_nodes+burst.after_nodes),'write burst repeated the entire dirty graph');
    check(burst.key_reads<=34,'write burst scanned intermediate source windows');
    await settle();
    const burstResult=await page.evaluate(()=>JSON.parse(window.demo.stats()));
    const burstRows=await page.locator('[role=listitem]').count();
    // This fixture retains the visible branch (unlike acceptance.js). One
    // fractional boundary may add one two-node row to the preceding window.
    check(Math.abs(burstResult.offset-burst.target)<0.001 && burstResult.keys-budget.keys<=68 && burstRows<=34 && burstResult.nodes<=budget.nodes+2,`burst window ${JSON.stringify({budget,burst,burstResult,burstRows})}`);
    await page.evaluate(()=>window.demo.profile_stop());
    const stopped=await page.evaluate(()=>JSON.parse(window.demo.profile_stats()));
    await page.evaluate(()=>{window.demo.increment();window.demo.seek(0);});await settle();
    const after=await page.evaluate(()=>JSON.parse(window.demo.profile_stats()));
    check(!after.enabled && JSON.stringify(after.phases)===JSON.stringify(stopped.phases),'disabled collector kept recording');
    await page.evaluate(()=>window.demo.profile_reset());
    const reset=await page.evaluate(()=>JSON.parse(window.demo.profile_stats()));
    check(Object.values(reset.phases).every(sample=>sample.calls===0),'reset left stale measurements');
    await page.evaluate(()=>window.demo.unmount());await settle();
    check(await page.locator('#app').evaluate(root=>root.childNodes.length===0),'profile retained UI after unmount');
    check(errors.length===0,errors.join('\n'));
    return {measured,budget,burst,burstResult,burstRows,browserSinglePass:true,disabledStable:true,reset:true,cleanup:true,errors};
  } finally {await context.close();}
}
