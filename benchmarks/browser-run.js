// Use a fresh foreground page/context for each framework. The same CDP meter
// surrounds each timed interval, excluding setup and correctness snapshots.
async (page) => {
  await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__&&document.querySelector('.bench-run'));
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  const app=await page.title();
  const cdp=await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  await cdp.send('HeapProfiler.collectGarbage');
  let initial;
  const fields=['ScriptDuration','LayoutDuration','RecalcStyleDuration','TaskDuration'];
  await page.exposeFunction('__BENCH_METRICS__',async boundary=>{
    const response=await cdp.send('Performance.getMetrics');
    const values=Object.fromEntries(response.metrics.map(metric=>[metric.name,metric.value]));
    if(!fields.every(field=>Number.isFinite(values[field])))throw Error('CPU metrics unavailable');
    if(boundary==='begin'){initial=values;return null;}
    const delta=Object.fromEntries(fields.map(field=>[field,(values[field]-initial[field])*1000]));
    if(Object.values(delta).some(value=>value<0||!Number.isFinite(value)))throw Error('invalid CPU counter delta');
    return delta;
  });
  const started=new Date().toISOString();
  const results=await page.evaluate(()=>window.__BENCH__.runAll());
  if(errors.length)throw Error(JSON.stringify(errors));
  await cdp.detach();
  return {app,started,finished:new Date().toISOString(),browser:page.context().browser().version(),viewport:page.viewportSize(),errors,...results};
}
