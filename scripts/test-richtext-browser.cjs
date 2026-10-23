const fs=require('node:fs'),path=require('node:path');
const artifacts=process.env.GLORY_TEST_ARTIFACT_DIR||'D:/BuildCache/glory-richtext/browser';
if(!/^[dD]:[\\/]BuildCache[\\/]/.test(process.env.TEMP||''))throw Error('Set TEMP/TMP to a D:/BuildCache directory before launching browsers');
fs.mkdirSync(artifacts,{recursive:true});
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
(async()=>{
 const browser=await chromium.launch({channel:'msedge',headless:true});
 try {
 const page=await browser.newPage();
 await page.setContent('<style>table{border-collapse:collapse}td{border:1px solid;padding:8px}#rich{width:500px;min-height:100px}p{white-space:pre-wrap}</style><div id="rich" contenteditable="true"></div>');
 const source=fs.readFileSync(path.join(__dirname,'../crates/web/src/browser/rich_text.js'),'utf8').replaceAll('export function ','function ');
 await page.addScriptTag({content:source+'\nglobalThis.richTest={writeRich,readRich,setSelectionRich,selectionRich,validateRich,pasteRich,taskRich,setMarksRich,beforeInputRich,compositionRich};'});
 const initial=await page.evaluate(()=>{
  const root=document.getElementById('rich'),m={bold:false,italic:false,strike:false,code:false,link:null};
  const leaf=(id,text,kind=['paragraph'])=>({id,kind,spans:[{text,marks:{...m}}],children:[]});
  const doc={root:{id:'0',kind:['root'],spans:[],children:[leaf('1','A😀中文'),{id:'2',kind:['list',null],spans:[],children:[{id:'3',kind:['item',true],spans:[],children:[leaf('4','task')]}]},{id:'5',kind:['table',['left','right']],spans:[],children:[{id:'6',kind:['head'],spans:[],children:[leaf('7','one',['cell']),leaf('8','two',['cell'])]}]}]}};
  richTest.writeRich(root,JSON.stringify(doc));const read=JSON.parse(richTest.readRich(root));
  if(JSON.stringify(read.document)!==JSON.stringify(doc))throw Error('roundtrip '+JSON.stringify(read.document));
  root.focus();richTest.setSelectionRich(root,JSON.stringify({anchor:{block:'1',offset:1},focus:{block:'1',offset:3}}));
  if(document.getSelection().toString()!=='😀')throw Error('UTF16 selection');
  const before=root.querySelector('[data-gly-rich-block="1"]');richTest.writeRich(root,JSON.stringify(doc));
  if(root.querySelector('[data-gly-rich-block="1"]')!==before||document.getSelection().toString()!=='😀')throw Error('controlled echo reset selection');
  let rejected=false;try{const bad=structuredClone(doc);bad.root.children[0].spans[0].marks.link='javascript:alert(1)';richTest.writeRich(root,JSON.stringify(bad));}catch{rejected=true;}if(!rejected)throw Error('unsafe URL');
  const bounds=root.querySelector('td').getBoundingClientRect();
  return {roundtrip:true,selection:document.getSelection().toString(),cell:[bounds.width,bounds.height],task:root.querySelector('li').dataset.glyTask,unsafeUrlRejected:true};
 });
 await page.keyboard.type('replacement');
 const typed=await page.evaluate(()=>JSON.parse(richTest.readRich(document.getElementById('rich'))));
 if(typed.document.root.children[0].spans[0].text!=='Areplacement中文')throw Error('native input replacement failed');
 await page.keyboard.press('Enter');
 const split=await page.evaluate(()=>{const root=document.getElementById('rich');const a=richTest.readRich(root),b=richTest.readRich(root);if(a!==b)throw Error('new DOM identities unstable');richTest.writeRich(root,JSON.stringify(JSON.parse(a).document));return JSON.parse(richTest.readRich(root));});
 if(split.document.root.children.filter(b=>b.kind[0]==='paragraph').length!==2)throw Error('Enter did not split the visual paragraph');
 await page.evaluate(()=>{
  const root=document.getElementById('rich');root.addEventListener('beforeinput',e=>richTest.beforeInputRich(root,e));root.addEventListener('compositionstart',()=>richTest.compositionRich(root));root.addEventListener('click',e=>richTest.taskRich(root,e));
  const doc=JSON.parse(richTest.readRich(root)).document,paragraph=doc.root.children.filter(b=>b.kind[0]==='paragraph')[1];
  root.focus();richTest.setSelectionRich(root,JSON.stringify({anchor:{block:paragraph.id,offset:0},focus:{block:paragraph.id,offset:0}}));richTest.setMarksRich(root,JSON.stringify({bold:true,italic:true,strike:false,code:false,link:null}));
 });
 await page.keyboard.type('formatted');
 const typing=await page.evaluate(()=>{const data=JSON.parse(richTest.readRich(document.getElementById('rich')));const spans=data.document.root.children[1].spans;if(!spans.some(s=>s.text?.includes('formatted')&&s.marks.bold&&s.marks.italic))throw Error('collapsed typing marks lost');return spans;});
 const cdp=await page.context().newCDPSession(page);await cdp.send('Input.imeSetComposition',{text:'你',selectionStart:1,selectionEnd:1});
 const preedit=await page.evaluate(()=>JSON.parse(richTest.readRich(document.getElementById('rich'))));
 if(!preedit.document.root.children[1].spans.some(s=>s.text?.includes('你')&&s.marks.bold))throw Error('preedit lost typing marks');
 await cdp.send('Input.insertText',{text:'你好'});
 const committed=await page.evaluate(()=>JSON.parse(richTest.readRich(document.getElementById('rich'))));
 if(!committed.document.root.children[1].spans.some(s=>s.text?.includes('你好')&&s.marks.bold))throw Error('IME commit lost formatting');
 await page.locator('input[data-gly-task-control]').click();
 const task=await page.locator('li[data-gly-task]').getAttribute('data-gly-task');if(task!=='false')throw Error('task click did not change finite document');
 await page.screenshot({path:path.join(artifacts,'rich-text-host.png')});
 console.log(JSON.stringify({initial,typed:typed.document.root.children[0].spans,split:split.document.root.children.map(b=>b.id),typing,imeCommitted:true,taskToggled:true},null,2));
 } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
