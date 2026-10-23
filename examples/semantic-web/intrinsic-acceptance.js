async (hostPage) => {
  const context = await hostPage.context().browser().newContext({ viewport: { width: 1000, height: 700 }, deviceScaleFactor: 1 });
  const page = await context.newPage();
  const errors = [], pending = new Map(), results = [];
  const png = {
    32: 'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAgCAYAAAAv8DnQAAAAGklEQVR4nGNg7LnzHx9mGFUwqmBUwf8RqgAAKXtoH1R/wlMAAAAASUVORK5CYII=',
    96: 'iVBORw0KGgoAAAANSUhEUgAAAAgAAABgCAYAAAAgqDO8AAAAKUlEQVR4nO3KQREAAAQAMPqLqAsNnAB77LfI6rmEIAiCIAiCIAjCvMICOzg4atGDdNkAAAAASUVORK5CYII=',
    128: 'iVBORw0KGgoAAAANSUhEUgAAAAgAAACACAYAAAA27Cg+AAAAK0lEQVR4nO3KMQEAAAQAMPqLqAsNPN4d+xZZPZcQBEEQBEEQBEEQBEGYV1gjeqCIoVN/+wAAAABJRU5ErkJggg=='
  };
  let automatic = false;
  const check = (value, message) => { if (!value) throw Error(message); };
  const settle = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const stats = () => page.evaluate(() => JSON.parse(window.demo.intrinsic_stats()));
  const list = page.getByRole('list', { name: 'Photos' });
  const row = key => list.locator(`[role=listitem][aria-posinset="${key+1}"]`);
  const rect = key => row(key).evaluate(element => {
    const a=element.getBoundingClientRect(), b=element.closest('[role=list]').getBoundingClientRect();
    return { top:a.top-b.top, bottom:a.bottom-b.top, height:a.height, viewport:b.height };
  });
  const fulfill = (route,height) => route.fulfill({status:200,contentType:'image/png',headers:{'cache-control':'no-store'},body:Buffer.from(png[height],'base64')});
  const finish = async (key,height,revision=0) => {
    const id=`${key}:${revision}`;
    for (let i=0;i<60 && !pending.has(id);i++) await settle();
    check(pending.has(id),`image request missing: ${id}`);
    const route=pending.get(id);pending.delete(id);await fulfill(route,height);
    await row(key).locator('img').evaluate(img => img.decode());await settle();await settle();
  };
  page.on('pageerror',error => errors.push(error.message));
  page.on('console',message => { if(message.type()==='error' && message.text().includes('Glory')) errors.push(message.text()); });
  try {
    await page.route('**/intrinsic-assets/*.png*', async route => {
      const url=new URL(route.request().url()), key=Number(url.pathname.split('/').pop().split('.')[0]);
      if(automatic) await fulfill(route,32).catch(()=>{});
      else pending.set(`${key}:${url.searchParams.get('v')}`,route);
    });
    await page.goto('http://127.0.0.1:8788/');await page.waitForFunction(()=>window.ready===true);
    await page.evaluate(()=>window.demo.intrinsic_mount());await settle();await settle();
    const key=1_000_000, before=await rect(key), anchor=await row(key).elementHandle();
    check(Math.abs(before.top+7)<1,`initial anchor ${JSON.stringify(before)}`);
    const logical=(await stats()).logical;
    await finish(key-1,96);
    check(Math.abs((await rect(key)).top-before.top)<1,'overscan image shifted anchor');
    check(Math.abs((await stats()).logical-logical-72)<1,'height delta was not reflected in logical offset');
    await finish(key,128);
    check(await row(key).evaluate((node,old)=>node===old,anchor),'image growth recreated anchor row');
    check(Math.abs((await rect(key)).height-128)<1 && Math.abs((await rect(key)).top-before.top)<1,'anchor image growth moved the viewport');
    await page.evaluate(key=>window.demo.intrinsic_replace(key),key);await settle();
    await finish(key,32,1);
    check(Math.abs((await rect(key)).height-32)<1 && Math.abs((await rect(key)).top-before.top)<1,'image replacement/shrink lost pixel inset');
    results.push('real PNG decode and ResizeObserver preserve row identity and 7px inset across overscan growth, anchor growth and source shrink');

    await list.focus();await page.keyboard.press('End');await settle();await settle();
    const last=99_999_999;
    const endState={stats:await stats(),rect:await rect(last),physical:await list.evaluate(element=>element.scrollTop)};
    check(await row(last).evaluate(element=>document.activeElement===element),'End did not focus last logical row');
    await finish(last,96);
    const revealed=await rect(last);
    check(revealed.top>=-1 && revealed.bottom<=revealed.viewport+1,`late navigation reveal ${JSON.stringify({revealed,endState,now:await stats(),physical:await list.evaluate(element=>element.scrollTop),focused:await row(last).evaluate(element=>document.activeElement===element)})}`);
    const controlled=await stats();check(Math.abs(controlled.echo-controlled.logical)<1,'controlled scroll echo diverged');
    const reports=controlled.reports;await settle();await settle();
    check((await stats()).reports===reports,'retained navigation produced idle feedback');
    results.push('End retains one adaptive reveal through a previously measured placeholder; controlled position echo converges without idle notifications');

    await list.evaluate(element=>element.scrollTop-=100);await settle();await settle();
    const user=(await stats()).logical;
    await page.addStyleTag({content:'.intrinsic-photo{width:16px}'});await settle();await settle();
    check(Math.abs((await stats()).logical-user)<1,'image resize overrode user scroll');
    await page.evaluate(()=>window.demo.intrinsic_seek(0));await settle();await settle();
    await page.addStyleTag({content:'.intrinsic-photo{width:32px}'});await settle();await settle();
    check(Math.abs((await stats()).logical)<1,'image resize overrode program position');
    await list.focus();await settle();
    await page.addStyleTag({content:'.intrinsic-photo{width:auto}'});
    automatic=true;
    for(const [id,route] of pending){pending.delete(id);await fulfill(route,32).catch(()=>{});}
    await settle();await settle();
    results.push('user scroll and program position retire the old reveal, including later intrinsic width/height changes');
    let maxRows=0,maxSlots=0,maxGraph=0;
    for(let i=0;i<40;i++){
      await page.evaluate(i=>window.demo.intrinsic_seek(100_000_007+i*19_123_000),i);await settle();await settle();
      const value=await stats();maxRows=Math.max(maxRows,value.rows);maxSlots=Math.max(maxSlots,value.slots);maxGraph=Math.max(maxGraph,value.graphSlots);
      check(value.rows<=17 && value.measured<=64 && value.heightCapacity<2000 && value.nodes<70 && value.graphSlots<300,`unbounded image window ${JSON.stringify(value)}`);
    }
    await page.evaluate(()=>window.demo.intrinsic_seek(24_000_007));await settle();await settle();
    check(!await anchor.evaluate(node=>node.isConnected),'offscreen anchor DOM was retained');
    check(Math.abs((await rect(key)).top-before.top)<1,'cached image return lost anchor');
    const final=await stats();check(final.keys<5000,`full-source work ${final.keys}`);check(errors.length===0,errors.join('\n'));
    await page.evaluate(()=>window.demo.unmount());await settle();check(await list.count()===0,'image list remained mounted');
    return {results,maxRows,maxSlots,maxGraph,final,errors};
  } finally { await context.close(); }
}
