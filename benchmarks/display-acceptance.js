// Beyond the shared row oracle: sparse runtime ownership, failures and browser state.
async page => {
 const check=(condition,message)=>{if(!condition)throw Error(message);};
 const reload=async()=>{await page.reload();await page.waitForFunction(()=>window.__BENCH_READY__&&window.__BENCH__);};
 const click=selector=>page.evaluate(async selector=>{document.querySelector(selector).click();await window.__BENCH__.settle();},selector);
 const results={};
 const initialStats=await page.evaluate(()=>window.__BENCH_STATS__());
 results.density=await page.evaluate(async()=>{
  const source=await (await fetch('./app.js')).text();
  const path=source.match(/from ['"]([^'"]*template_host\.js)['"]/)[1];
  const hostApi=await import(new URL(path,location.href));
  return [1,200].map(count=>{
   const root=document.createElement('div'),host=hostApi.createHost(root,()=>{});
   const children=[[0,''],...Array.from({length:count-1},()=>[1,'span',[],[]])];
   const id=hostApi.defineTemplate(host,JSON.stringify([[[1,'div',[],children]],[[[0,0],0,null],[[0],3,null]]]));
   const reply=hostApi.submit(host,new Uint32Array([1,1,id,0,0,0,0,0,3,1,0,0,0,0,0,0]),'');
   if(reply[0]!==2||reply[1]!==null)throw Error('density create/place failed');
   const walker=document.createTreeWalker(root,NodeFilter.SHOW_ALL);let physical=0;while(walker.nextNode())physical++;
   const result={physical,instances:host.instances.size,targets:host.instances.get(1).targets.length};
   hostApi.destroyHost(host);return result;
  });
 });
 check(JSON.stringify(results.density)==='[{"physical":2,"instances":1,"targets":2},{"physical":201,"instances":1,"targets":2}]','static growth changed indexed host records or omitted physical nodes');
 results.wide=await page.evaluate(async()=>{
  const source=await (await fetch('./app.js')).text();
  const path=source.match(/from ['"]([^'"]*template_host\.js)['"]/)[1];
  const api=await import(new URL(path,location.href));
  const root=document.createElement('div'),host=api.createHost(root,()=>{}),count=200;
  const children=Array.from({length:count},()=>[1,'span',[],[[0,'']]]);
  const targets=children.map((_,index)=>[[0,index,0],0,null]);
  const template=api.defineTemplate(host,JSON.stringify([[[1,'div',[],children]],targets]));
  const sibling=Object.getOwnPropertyDescriptor(Node.prototype,'nextSibling');let reads=0;
  Object.defineProperty(Node.prototype,'nextSibling',{...sibling,get(){reads++;return sibling.get.call(this);}});
  let reply;
  try{reply=api.submit(host,new Uint32Array([1,1,template,0,0,0,0,0,3,1,0,0,0,0,0,0]),'');}
  finally{Object.defineProperty(Node.prototype,'nextSibling',sibling);}
  if(reply[0]!==2||reply[1]!==null)throw Error('wide target creation failed');
  const instance=host.instances.get(1);
  const identity=instance.targets.every((target,index)=>target===root.firstChild.childNodes[index].firstChild);
  const words=[];for(let slot=0;slot<count;slot++)words.push(2,1,slot,0,1,0,0,0);
  reply=api.submit(host,new Uint32Array(words),'x');
  const writes=reply[0]===count&&reply[1]===null&&root.textContent==='x'.repeat(count);
  api.destroyHost(host);return {count,reads,identity,writes};
 });
 check(results.wide.identity&&results.wide.writes&&results.wide.reads<=4*results.wide.count,'wide template target lookup/identity regressed '+JSON.stringify(results.wide));
 results.probes=await page.evaluate(async()=>{
  const app=await import('./app.js');return app.probe_templates?JSON.parse(app.probe_templates()):null;
 });
 if(results.probes)check(JSON.stringify(results.probes.live)==='[[1,2,1],[1,2,1]]','static growth changed live record counts');
 for(const fallback of [false,true]) for(const focusedIndex of [1,998]) {
  await reload();await click('.bench-run');
  await page.evaluate(({fallback,focusedIndex})=>{
   window.__moveBefore=Object.getOwnPropertyDescriptor(Element.prototype,'moveBefore');
   if(fallback)Object.defineProperty(Element.prototype,'moveBefore',{value:undefined,configurable:true});
   const label=document.querySelectorAll('.bench-row')[focusedIndex].querySelector('.lbl');
   window.__destination=focusedIndex===1?998:1;
   window.__focused=label;label.focus();const text=label.firstChild;
   document.getSelection().setBaseAndExtent(text,1,text,5);
   window.__getSelection=Document.prototype.getSelection;window.__selectionReads=0;
   Document.prototype.getSelection=function(){window.__selectionReads++;return window.__getSelection.call(this)};
  },{fallback,focusedIndex});
  await click('.bench-swaprows');
  check(await page.evaluate(()=>{Document.prototype.getSelection=window.__getSelection;return window.__selectionReads===1}),'selection was not snapshotted once per move run');
  check(await page.evaluate(()=>{
   const label=window.__focused,selection=document.getSelection();
   return document.activeElement===label&&document.querySelectorAll('.bench-row')[window.__destination].querySelector('.lbl')===label&&selection.anchorNode===label.firstChild&&selection.anchorOffset===1&&selection.focusOffset===5;
  }),'move lost focus, text identity or selection '+JSON.stringify({fallback,...await page.evaluate(()=>{const s=document.getSelection();return {active:document.activeElement?.outerHTML,expected:window.__focused?.outerHTML,anchor:s.anchorNode?.nodeName,ao:s.anchorOffset,fo:s.focusOffset}})}));
  await page.evaluate(()=>{if(window.__moveBefore)Object.defineProperty(Element.prototype,'moveBefore',window.__moveBefore);});
 }
 results.focus={atomic:true,fallback:true};
 await reload();
 results.grow=await page.evaluate(async()=>{
  const app=await import('./app.js'),wasm=await app.default(),original=Node.prototype.cloneNode;
  let calls=0,reentry;
  Node.prototype.cloneNode=function(deep){if(++calls===1){wasm.memory.grow(1);reentry=app.flush_template();}return original.call(this,deep);};
  try {document.querySelector('.bench-run').click();await window.__BENCH__.settle();}
  finally {Node.prototype.cloneNode=original;}
  return {calls,reentry,stats:window.__BENCH_STATS__()};
 });
 check(results.grow.calls===1000&&results.grow.reentry===false&&results.grow.stats.instances===1001,'memory growth/reentry corrupted a packet '+JSON.stringify(results.grow));
 await reload();
 results.createFailure=await page.evaluate(async()=>{
  const app=await import('./app.js'),original=Node.prototype.cloneNode;let calls=0;
  Node.prototype.cloneNode=function(deep){if(++calls===300)throw Error('injected create 300');return original.call(this,deep);};
  try {document.querySelector('.bench-run').click();await window.__BENCH__.settle();}
  finally {Node.prototype.cloneNode=original;}
  const stats=window.__BENCH_STATS__(),physical=document.querySelectorAll('[role="row"]').length;
  const retry=app.flush_template();document.querySelector('.bench-run').click();await window.__BENCH__.settle();
  const after=window.__BENCH_STATS__(),closed=JSON.parse(app.close_template());
  return {calls,stats,physical,retry,after,closed,children:document.getElementById('main').childNodes.length};
 });
 const created=results.createFailure;
 // Cache mounts first, then places: the successful create prefix is detached.
 check(created.calls===300&&created.physical===0&&created.stats.instances===300&&created.stats.targets===1502,'create prefix miscount '+JSON.stringify(created));
 check(!created.retry&&JSON.stringify(created.stats)===JSON.stringify(created.after),'faulted document accepted a suffix/event');
 check(created.closed.instances===0&&created.closed.records===0&&created.closed.graph===0&&created.closed.owners===1&&created.children===0,'faulted create leaked teardown state');
 await reload();
 results.placeFailure=await page.evaluate(async()=>{
  const app=await import('./app.js'),original=Node.prototype.insertBefore;let calls=0;
  Node.prototype.insertBefore=function(node,before){if(this.classList?.contains('bench-body')&&++calls===300)throw Error('injected place 300');return original.call(this,node,before);};
  try {document.querySelector('.bench-run').click();await window.__BENCH__.settle();}
  finally {Node.prototype.insertBefore=original;}
  const stats=window.__BENCH_STATS__(),physical=document.querySelectorAll('[role="row"]').length;
  const retry=app.flush_template(),closed=JSON.parse(app.close_template());return {calls,stats,physical,retry,closed};
 });
 check(results.placeFailure.calls===300&&results.placeFailure.physical===299&&results.placeFailure.stats.instances===1001&&!results.placeFailure.retry&&results.placeFailure.closed.records===0,'place prefix/suffix failure '+JSON.stringify(results.placeFailure));
 await reload();await click('.bench-run');
 results.writeFailure=await page.evaluate(async()=>{
  const app=await import('./app.js'),descriptor=Object.getOwnPropertyDescriptor(Node.prototype,'nodeValue');let calls=0;
  Object.defineProperty(Node.prototype,'nodeValue',{...descriptor,set(value){if(++calls===20)throw Error('injected write 20');descriptor.set.call(this,value);}});
  try {document.querySelector('.bench-update').click();await window.__BENCH__.settle();}
  finally {Object.defineProperty(Node.prototype,'nodeValue',descriptor);}
  const changed=[...document.querySelectorAll('.lbl')].filter(label=>label.textContent.endsWith(' !!!')).length;
  const retry=app.flush_template(),closed=JSON.parse(app.close_template());return {calls,changed,retry,closed};
 });
 check(results.writeFailure.calls===20&&results.writeFailure.changed===19&&!results.writeFailure.retry&&results.writeFailure.closed.records===0,'write prefix/suffix failure '+JSON.stringify(results.writeFailure));
 await reload();await click('.bench-run');
 results.retireFailure=await page.evaluate(async()=>{
  const app=await import('./app.js'),original=Element.prototype.remove;let calls=0;
  Element.prototype.remove=function(){if(++calls===17)throw Error('injected retire 17');return original.call(this);};
  try {document.querySelector('.bench-clear').click();await window.__BENCH__.settle();}
  finally {Element.prototype.remove=original;}
  const stats=window.__BENCH_STATS__(),physical=document.querySelectorAll('[role="row"]').length;
  const retry=app.flush_template(),closed=JSON.parse(app.close_template());return {calls,stats,physical,retry,closed};
 });
 const retired=results.retireFailure;
 check(retired.calls===17&&retired.physical===984&&retired.stats.instances===985&&retired.stats.targets===4927&&retired.stats.records===1&&retired.stats.graph===initialStats.graph,'retire prefix miscount '+JSON.stringify(retired));
 check(!retired.retry&&retired.closed.records===0&&retired.closed.owners===1,'retire suffix/cleanup failure');
 await reload();await click('.bench-run');
 await page.evaluate(()=>{window.__goneRoot=new WeakRef(document.querySelector('.bench-row'));window.__goneText=new WeakRef(document.querySelector('.lbl').firstChild);});
 await click('.bench-clear');await page.evaluate(()=>window.__BENCH__.settle());
 const cdp=await page.context().newCDPSession(page);
 for(let i=0;i<4;i++)await cdp.send('HeapProfiler.collectGarbage');await cdp.detach();
 results.collected=await page.evaluate(()=>!window.__goneRoot.deref()&&!window.__goneText.deref());
 check(results.collected,'retired row/text retained after rendering and four GCs');
 return results;
}
