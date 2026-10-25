// Independent JS business oracle. All verification runs outside timing windows.
(() => {
  const n = window.scenarioCount();
  const money = v => Math.floor(v / 100) + '.' + String(v % 100).padStart(2, '0');
  const settle = async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); };
  let expected = [], total = 0, step = 0, selectionStep = 0, selected = null, identities = [];
  const value = q => q.price * q.quantity;
  function tick(count, repeats) {
    if (!expected.length) return;
    for (let r = 0; r < repeats; r++) {
      step++;
      for (let offset = 0; offset < Math.min(count, n); offset++) {
        const index = (step * 73 + offset) % n, q = expected[index];
        total -= value(q);
        q.price = 10000 + index * 37 % 20000 + (step * 17 + index * 13) % 1999;
        q.quantity = (step * 3 + index * 7) % 100 + 1;
        q.up = (step + index) % 2 === 0;
        total += value(q);
      }
    }
  }
  function advance(op) {
    if (op === 'mount') {
      if (expected.length) return;
      expected = Array.from({length:n}, (_,index) => ({index,price:10000+index*37%20000,quantity:index%99+1,up:true,selected:false}));
      total = expected.reduce((v,q)=>v+value(q),0);step=selectionStep=0;selected=null;
    } else if (op === 'clear') { expected=[]; total=step=selectionStep=0;selected=null; }
    else if (op === 'select') {
      if (!expected.length) return;
      if (selected !== null) expected[selected].selected=false;
      selected=(++selectionStep*97)%n;expected[selected].selected=true;
    } else {
      const count={one:1,sparse:n/100,medium:n/10,dense:n,burst:n/100}[op];
      if (count===undefined) throw Error('unknown operation');
      tick(count,op==='burst'?10:1);
    }
  }
  const rows=()=>[...document.querySelectorAll('.quotes > .quote')];
  function track() {
    identities = rows().flatMap(row => {
      const list=[row], walker=document.createTreeWalker(row,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT);
      while(walker.nextNode()) list.push(walker.currentNode);
      return list.map(node=>[node,node.nodeType===1?node.getAttribute('data-gly-node'):null]);
    });
  }
  async function verify(op, keepIdentity=true) {
    advance(op);
    const live=rows();
    if(live.length!==expected.length)throw Error('row count '+live.length+' != '+expected.length);
    const actual=live.map((row,index)=>{
      const q=expected[index], fields=[...row.children];
      if(row.tagName!=='DIV'||row.getAttribute('role')!=='row'||fields.length!==4||fields.some(f=>f.tagName!=='SPAN'))throw Error('shape');
      const texts=fields.map(f=>f.textContent), want=['ASSET '+String(index+1).padStart(5,'0'),money(q.price),String(q.quantity),money(value(q))];
      const wantClass='quote '+(q.up?'up':'down')+(q.selected?' selected':'');
      if(JSON.stringify(texts)!==JSON.stringify(want)||row.className!==wantClass)throw Error('data mismatch '+index+' '+JSON.stringify({texts,want,cls:row.className,wantClass}));
      return [...texts,row.className];
    });
    if(document.querySelector('.total').textContent!==money(total))throw Error('portfolio total');
    if(keepIdentity && op!=='mount' && op!=='clear')for(const [node,id] of identities) {
      if(!node.isConnected || (node.nodeType===1&&node.getAttribute('data-gly-node')!==id))throw Error('identity changed');
    }
    if(op==='clear') { if(identities.some(([node])=>node.isConnected))throw Error('old node still connected');identities=[]; }
    if(op==='mount'&&keepIdentity)track();
    const stats=window.scenarioStats();
    const granular=location.pathname.includes('glory')||location.pathname.includes('dioxus-row');
    if(stats.registry!==(granular?expected.length:0))throw Error('live registry');
    const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(JSON.stringify({actual,total:money(total)})));
    return {count:live.length,sha256:[...new Uint8Array(bytes)].map(b=>b.toString(16).padStart(2,'0')).join(''),stats};
  }
  window.scenario={settle,verify,track,release(){identities=[];},
    async click(op){if(document.visibilityState!=='visible')throw Error('hidden benchmark');const start=performance.now();document.querySelector('button.'+op).click();await settle();return performance.now()-start;},
    async run(op,identity=true){await this.click(op);return verify(op,identity);},
    geometry(){return rows().map(row=>{const r=row.getBoundingClientRect();return [r.x,r.y,r.width,r.height,...[...row.children].flatMap(c=>{const b=c.getBoundingClientRect();return [b.x,b.y,b.width,b.height];})];});},
    summaryGeometry(){const e=document.querySelector('.total'),range=document.createRange();range.selectNodeContents(e);return{element:e.getBoundingClientRect().toJSON(),text:range.getBoundingClientRect().toJSON(),numeric:getComputedStyle(e).fontVariantNumeric};},
    baseline(){return window.scenarioStats();}
  };
})();
