// Supplement the shared seeded-data oracle with descendant identity and geometry.
async page => {
 const steps=[];
 const baseline=await page.evaluate(()=>window.__BENCH_STATS__?.()??null);
 const click=async selector=>page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},selector);
 await click('.bench-run');
 await page.evaluate(()=>{
  window.__retained=new Map([...document.querySelectorAll('.bench-row')].map(row=>{
   const elements=[row,...row.querySelectorAll('*')],walker=document.createTreeWalker(row,NodeFilter.SHOW_TEXT),texts=[];
   while(walker.nextNode())texts.push(walker.currentNode);
   return [+row.children[0].textContent,{row,elements,texts,tokens:elements.map(element=>element.getAttribute('data-gly-node'))}];
  }));
 });
 for(const [name,selector] of [['update','.bench-update'],['select','.bench-row:nth-child(2) .lbl'],['swap','.bench-swaprows'],['remove','.bench-row:nth-child(2) .remove'],['append','.bench-add']]) {
  if(name==='remove')await page.evaluate(()=>window.__removed=document.querySelectorAll('.bench-row')[1]);
  await click(selector);
  const retained=await page.evaluate(()=>{
   const rows=[...document.querySelectorAll('.bench-row')];
   for(const row of rows){
    const old=window.__retained.get(+row.children[0].textContent);if(!old)continue;
    const elements=[row,...row.querySelectorAll('*')],walker=document.createTreeWalker(row,NodeFilter.SHOW_TEXT),texts=[];
    while(walker.nextNode())texts.push(walker.currentNode);
    if(row!==old.row||elements.length!==old.elements.length||texts.length!==old.texts.length||elements.some((element,i)=>element!==old.elements[i]||element.getAttribute('data-gly-node')!==old.tokens[i])||texts.some((text,i)=>text!==old.texts[i]))return false;
   }
   return !window.__removed||!window.__removed.isConnected;
  });
  if(!retained)throw Error(name+': replaced surviving row, descendant, text node or semantic identity');
  steps.push({name,retained,...await page.evaluate(()=>window.__BENCH__.snapshot())});
 }
 await click('.bench-run');
 if(!await page.evaluate(()=>[...window.__retained.values()].every(old=>old.elements.every(element=>!element.isConnected)&&old.texts.every(text=>!text.isConnected))))throw Error('replacement retained old descendants');
 await click('.bench-runlots');
 const geometry=await page.evaluate(async()=>{
  const rows=[...document.querySelectorAll('.bench-row')];
  const rectangles=rows.map(row=>{const rect=row.getBoundingClientRect();return [Number(row.children[0].textContent),rect.x,rect.y,rect.width,rect.height];});
  if(rows.length!==10000||rectangles.some(rect=>rect[4]<=0))throw Error('10000 real row boxes required');
  const bytes=new TextEncoder().encode(JSON.stringify(rectangles)),hash=await crypto.subtle.digest('SHA-256',bytes);
  return {rows:rows.length,sha256:[...new Uint8Array(hash)].map(byte=>byte.toString(16).padStart(2,'0')).join(''),first:rectangles[0],last:rectangles.at(-1)};
 });
 await click('.bench-clear');
 const stats=await page.evaluate(()=>window.__BENCH_STATS__?.()??null);
 if(JSON.stringify(stats)!==JSON.stringify(baseline))throw Error('clear retained live row state');
 if(stats?.profile==='html-display-v1'&&(stats.instances!==1||stats.targets!==7||stats.records!==1||stats.graph!==3||stats.owners!==2))throw Error('invalid display profile cleanup');
 if(stats?.profile==='html-display-keyed-v2'&&(stats.instances!==1||stats.targets!==7||stats.records!==1||stats.graph!==5||stats.owners!==2))throw Error('invalid keyed display profile cleanup');
 return {steps,geometry,cleanup:stats};
}
