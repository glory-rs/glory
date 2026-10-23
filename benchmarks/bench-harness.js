// Local semantic-rows-v1 workload. Not the official HTML-table benchmark.
// Timing is click to two foreground RAF callbacks, not presentation latency.
export const WARMUP = 3;
export const SAMPLES = 12;
const $ = selector => document.querySelector(selector);
const rows = () => [...document.querySelectorAll('.bench-body > .bench-row')];
const click = selector => { const element = typeof selector === 'string' ? $(selector) : selector; if (!element) throw Error('missing '+selector); element.click(); };
const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
export async function settle() { await frame(); await frame(); }
export const operations = [
  {key:'create_1k', setup:'.bench-clear', action:'.bench-run', count:1000},
  {key:'replace_1k', setup:'.bench-run', action:'.bench-run', count:1000},
  {key:'update_10th', setup:'.bench-run', action:'.bench-update', count:1000},
  {key:'select_row', setup:'.bench-run', action:() => click(rows()[1].querySelector('.lbl')), count:1000},
  {key:'swap_rows', setup:'.bench-run', action:'.bench-swaprows', count:1000},
  {key:'remove_row', setup:'.bench-run', action:() => click(rows()[1].querySelector('.remove')), count:999},
  {key:'append_1k', setup:'.bench-run', action:'.bench-add', count:2000},
  {key:'create_10k', setup:'.bench-clear', action:'.bench-runlots', count:10000},
  {key:'clear_1k', setup:'.bench-run', action:'.bench-clear', count:0},
];
export async function snapshot() {
  const records = rows().map(row => {
    const cells = [...row.children];
    if (row.tagName!=='DIV' || row.getAttribute('role')!=='row' || cells.length!==4 || cells.some(cell=>cell.tagName!=='DIV'||cell.getAttribute('role')!=='cell')) throw Error('row/cell contract');
    if (cells[0].children.length!==1 || cells[0].firstElementChild.tagName!=='SPAN' || cells[1].children.length!==1 || cells[1].firstElementChild.tagName!=='BUTTON' || cells[2].children.length!==1 || cells[2].firstElementChild.tagName!=='BUTTON' || cells[3].children.length!==0) throw Error('content contract');
    return [+cells[0].textContent, cells[1].textContent, row.classList.contains('danger')];
  });
  const hash = await crypto.subtle.digest('SHA-256',new TextEncoder().encode(JSON.stringify(records)));
  return {count:records.length,sha256:[...new Uint8Array(hash)].map(byte=>byte.toString(16).padStart(2,'0')).join('')};
}
async function sample(operation, measured) {
  click(operation.setup); await settle();
  if(document.visibilityState!=='visible') throw Error('benchmark must remain foreground');
  if(measured && window.__BENCH_METRICS__) await window.__BENCH_METRICS__('begin');
  const start=performance.now();
  if(typeof operation.action==='function') operation.action(); else click(operation.action);
  await settle();
  const elapsed=performance.now()-start;
  const metrics=measured && window.__BENCH_METRICS__ ? await window.__BENCH_METRICS__('end') : null;
  const result=await snapshot();
  if(result.count!==operation.count) throw Error(operation.key+': wrong row count '+result.count);
  if(window.__BENCH_STATS__) { const stats=window.__BENCH_STATS__(); if(stats.labels!==result.count || stats.rows!==result.count) throw Error('retired row state retained'); }
  return {elapsed_ms:elapsed,metrics,...result};
}
function median(values) { const sorted=values.slice().sort((a,b)=>a-b); return (sorted[5]+sorted[6])/2; }
export async function runAll() {
  const results=[];
  for(const operation of operations) {
    for(let i=0;i<WARMUP;i++) await sample(operation,false);
    const samples=[];
    for(let i=0;i<SAMPLES;i++) samples.push(await sample(operation,true));
    results.push({key:operation.key,median_ms:median(samples.map(x=>x.elapsed_ms)),samples});
    window.__BENCH_PROGRESS__=results.map(x=>({key:x.key,median_ms:x.median_ms}));
  }
  click('.bench-clear'); await settle();
  const value={schema:'semantic-rows-v1',warmup:WARMUP,samples:SAMPLES,results,final:await snapshot(),stats:window.__BENCH_STATS__?.()};
  window.__BENCH_RESULTS__=value;
  window.dispatchEvent(new CustomEvent('bench-done',{detail:value}));
  return value;
}
window.__BENCH__={runAll,settle,snapshot};
