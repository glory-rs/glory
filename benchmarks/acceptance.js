// Navigate the supplied page to /{glory,leptos,dioxus}/ before invoking.
async (page) => {
  await page.waitForFunction(() => window.__BENCH_READY__ && window.__BENCH__ && document.querySelector('.bench-run'));
  const errors=[]; page.on('pageerror',error=>errors.push(error.message));
  const app=await page.title();
  const snapshots=[];
  let random=0x2545_F491_4F6C_DD1Dn, next=1, expected=[], selected=null;
  const adjectives=['pretty','large','big','small','tall','short','long','handsome','plain','quaint','clean','elegant','easy','angry','crazy','helpful','mushy','odd','unsightly','adorable','important','inexpensive','cheap','expensive','fancy'];
  const colours=['red','yellow','blue','green','pink','brown','purple','brown','white','black','orange'];
  const nouns=['table','chair','house','bbq','desk','car','pony','cookie','sandwich','burger','pizza','mouse','keyboard'];
  const draw=words=>{random=BigInt.asUintN(64,random*6364136223846793005n+1442695040888963407n);return words[Number(random>>33n)%words.length];};
  const make=count=>Array.from({length:count},()=>[next++,`${draw(adjectives)} ${draw(colours)} ${draw(nouns)}`]);
  const click=async selector=>{await page.evaluate(selector=>document.querySelector(selector).click(),selector);await page.evaluate(()=>window.__BENCH__.settle());};
  async function check(stage) {
    const actual=await page.evaluate(()=>[...document.querySelectorAll('.bench-body > .bench-row')].map(row=>[+row.children[0].textContent,row.children[1].textContent,row.classList.contains('danger')]));
    const wanted=expected.map(([id,label])=>[id,label,id===selected]);
    if(JSON.stringify(actual)!==JSON.stringify(wanted)) throw Error(app+' '+stage+': row data differs');
    const state=await page.evaluate(()=>window.__BENCH__.snapshot());
    const stats=await page.evaluate(()=>window.__BENCH_STATS__?.());
    if(stats && (stats.rows!==expected.length || stats.labels!==expected.length))throw Error('row label lifetime');
    snapshots.push({stage,...state,stats});
  }
  const baseline=await page.evaluate(()=>window.__BENCH_STATS__?.());
  await check('empty');
  expected=make(1000);await click('.bench-run');await check('create_1k');
  const geometry=await page.evaluate(()=>[...document.querySelectorAll('.bench-row')].slice(0,3).map(row=>{const b=row.getBoundingClientRect();return [b.x,b.y,b.width,b.height];}));
  await page.screenshot({path:`${page.__BENCH_ARTIFACT_DIR || 'D:/BuildCache/glory-framework-compare'}/${app}-conformance.png`});
  await page.evaluate(()=>{window.__benchRows=[...document.querySelectorAll('.bench-row')];});
  expected=expected.map(([id,label],i)=>[id,label+(i%10===0?' !!!':'')]);await click('.bench-update');await check('update_10th');
  selected=expected[1][0];await click('.bench-row:nth-child(2) .lbl');await check('select_row');
  [expected[1],expected[998]]=[expected[998],expected[1]];await click('.bench-swaprows');await check('swap_rows');
  if(!await page.evaluate(()=>{const rows=[...document.querySelectorAll('.bench-row')];return rows[1]===window.__benchRows[998]&&rows[998]===window.__benchRows[1];}))throw Error('swap replaced keyed nodes');
  expected.splice(1,1);await click('.bench-row:nth-child(2) .remove');await check('remove_row');
  expected.push(...make(1000));await click('.bench-add');await check('append_1k');
  expected=make(1000);await click('.bench-run');await check('replace_1k');
  if(!await page.evaluate(()=>window.__benchRows.every(row=>!row.isConnected)))throw Error('replacement left old DOM live');
  expected=make(10000);await click('.bench-runlots');await check('create_10k');
  const mounted=await page.evaluate(()=>[...document.querySelectorAll('.bench-row')].every(row=>row.getBoundingClientRect().height>0));
  if(!mounted)throw Error('all 10000 rows must have layout boxes');
  const census=await page.evaluate(()=>{
    const root=document.getElementById('main'),walker=document.createTreeWalker(root,NodeFilter.SHOW_ALL);
    const counts={elements:0,text:0,comments:0,attributes:0};
    while(walker.nextNode()) {const node=walker.currentNode;if(node.nodeType===1){counts.elements++;counts.attributes+=node.attributes.length;}else if(node.nodeType===3)counts.text++;else if(node.nodeType===8)counts.comments++;}
    return {...counts,stats:window.__BENCH_STATS__?.()};
  });
  expected=[];selected=null;await click('.bench-clear');await check('clear_10k');
  for(let i=0;i<3;i++){
    expected=make(1000);await click('.bench-run');expected=[];await click('.bench-clear');await check('cleanup_'+i);
    const stats=await page.evaluate(()=>window.__BENCH_STATS__?.());
    if(stats && JSON.stringify(stats)!==JSON.stringify(baseline))throw Error('live reactive state grew after clear');
  }
  if(errors.length)throw Error(JSON.stringify(errors));
  return {app,snapshots,geometry,mounted,census,errors,browser:page.context().browser().version()};
}
