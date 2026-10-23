// Finite rich-document DOM adapter. No application callbacks or arbitrary HTML.
const states = new WeakMap();
const blankMarks = () => ({bold:false,italic:false,strike:false,code:false,link:null});
const canonical = value => value.replace(/\r\n?/g,'\n').replace(/\0/g,'\ufffd');
const blockTags = new Set(['P','DIV','H1','H2','H3','H4','H5','H6','BLOCKQUOTE','UL','OL','LI','PRE','HR','TABLE','THEAD','TBODY','TR','TD','TH']);
function state(root) {
  let value=states.get(root);
  if(!value){value={next:1n,ids:new WeakMap(),last:null};states.set(root,value);}
  return value;
}
function id(element,s,seen) {
  let value=element.getAttribute?.('data-gly-rich-block') || s.ids.get(element);
  if(value&&seen.has(value))value=s.ids.get(element);
  if(!value || !/^[1-9][0-9]*$/.test(value) || BigInt(value)>18446744073709551615n || seen.has(value)) {
    while(seen.has(s.next.toString()))s.next++;
    if(s.next>18446744073709551615n)throw Error('Rich document identity exhausted');
    value=(s.next++).toString();
  }
  seen.add(value);s.ids.set(element,value);s.next=s.next>BigInt(value)?s.next:BigInt(value)+1n;return value;
}
export function validateRich(document) {
  const ids=new Set(),stack=[[document?.root,0]],encoder=new TextEncoder();let bytes=0,spans=0,editable=false;
  const text=value=>{if(typeof value!=='string'||/[\r\0]/.test(value))throw Error('Invalid rich text');bytes+=encoder.encode(value).length;if(bytes>1048576)throw Error('Rich document byte budget');};
  const url=(value,image)=>{text(value);if(!value||value.length>8192||/\s|[\x00-\x1f\x7f]/.test(value))throw Error('Invalid rich URL');
    if(!/^(https?:\/\/|\/[^/])/.test(value)&&!(image?/^(blob:|asset:\/\/)/.test(value):/^mailto:/.test(value)))throw Error('Unsafe rich URL');};
  if(document?.root?.id!=='0'||document.root.kind?.[0]!=='root'||!document.root.children?.length)throw Error('Invalid rich root');
  while(stack.length){const [b,depth]=stack.pop();if(!b||depth>32||typeof b.id!=='string'||!/^(0|[1-9][0-9]*)$/.test(b.id)||BigInt(b.id)>18446744073709551615n||ids.has(b.id))throw Error('Invalid rich block identity');ids.add(b.id);
    if(ids.size>8192||!Array.isArray(b.kind)||!Array.isArray(b.spans)||!Array.isArray(b.children)||ids.size+stack.length+b.children.length>8192)throw Error('Rich structure budget');
    const [kind,arg]=b.kind,leaf=['paragraph','heading','code','cell'].includes(kind)||kind==='item'&&!b.children.length;
    editable ||= leaf && !b.children.length;
    if(!['root','paragraph','heading','quote','list','item','code','rule','table','head','row','cell'].includes(kind))throw Error('Unknown rich kind');
    if(kind==='heading'&&(!Number.isInteger(arg)||arg<1||arg>6))throw Error('Invalid rich heading');
    if(kind==='code')text(arg);
    if(kind==='list'&&arg!==null&&(typeof arg!=='string'||!/^\d+$/.test(arg)||BigInt(arg)>18446744073709551615n))throw Error('Invalid list start');
    if(kind==='item'&&arg!==null&&typeof arg!=='boolean')throw Error('Invalid task');
    if(kind==='table'&&(!Array.isArray(arg)||!arg.length||arg.length>64||arg.some(a=>!['none','left','center','right'].includes(a))))throw Error('Invalid table');
    if(kind==='table'&&(!b.children.length||b.children.some((row,i)=>row.children.length!==arg.length||row.kind[0]==='head'&&i!==0)))throw Error('Invalid rich table rows');
    if(b.spans.length&&(!leaf||b.children.length))throw Error('Rich spans require a leaf');
    spans+=b.spans.length;if(spans>32768)throw Error('Rich span budget');
    for(const span of b.spans){if(typeof span.text==='string'){text(span.text);const m=span.marks;if(!m||['bold','italic','strike','code'].some(k=>typeof m[k]!=='boolean'))throw Error('Invalid rich marks');if(m.link!==null)url(m.link,false);if(kind==='code'&&(m.bold||m.italic||m.strike||m.code||m.link))throw Error('Code marks');}
      else if(span.image){if(kind==='code')throw Error('Code image');url(span.image.source,true);text(span.image.alternative);text(span.image.title);}
      else if(span.break!==true)throw Error('Invalid rich span');}
    const allowed=['root','quote','item'].includes(kind)?['paragraph','heading','quote','list','code','rule','table']:kind==='list'?['item']:kind==='table'?['head','row']:['head','row'].includes(kind)?['cell']:[];
    for(const child of b.children){if(!allowed.includes(child.kind?.[0]))throw Error('Invalid rich child');stack.push([child,depth+1]);}
  }
  if(!editable)throw Error('Rich document requires an editable leaf');
  return true;
}
function spanText(spans,text,marks) {
  text=canonical(text);if(!text)return;
  const last=spans.at(-1);
  if(last?.text!==undefined && JSON.stringify(last.marks)===JSON.stringify(marks))last.text+=text;
  else spans.push({text,marks:{...marks}});
}
function readModel(root) {
  const s=state(root),seen=new Set(),bindings=new Map();let count=0,bytes=0;
  function inline(node,spans,marks,depth) {
    if(depth>32 || ++count>65536)throw Error('Rich document exceeds limits');
    if(node.nodeType===3){bytes+=node.data.length;if(bytes>1048576)throw Error('Rich text exceeds limit');spanText(spans,node.data,marks);return;}
    if(node.nodeType!==1)return;
    const tag=node.tagName;
    if(tag==='SCRIPT'||tag==='STYLE'||tag==='IFRAME'||tag==='OBJECT'||tag==='TEMPLATE')return;
    if(tag==='BR'){if(!node.hasAttribute('data-gly-rich-empty'))spans.push({break:true});return;}
    if(tag==='IMG'){spans.push({image:{source:node.getAttribute('src')||'',alternative:node.getAttribute('alt')||'',title:node.getAttribute('title')||''}});return;}
    marks={...marks};
    if(tag==='B'||tag==='STRONG')marks.bold=true;
    if(tag==='I'||tag==='EM')marks.italic=true;
    if(tag==='S'||tag==='STRIKE'||tag==='DEL')marks.strike=true;
    if(tag==='CODE')marks.code=true;
    if(tag==='A')marks.link=node.getAttribute('href');
    if(tag==='SPAN') {
      if(node.style.fontWeight==='bold'||Number(node.style.fontWeight)>=600)marks.bold=true;
      if(node.style.fontStyle==='italic')marks.italic=true;
      if(node.style.textDecorationLine.includes('line-through'))marks.strike=true;
    }
    for(const child of node.childNodes)inline(child,spans,marks,depth+1);
  }
  function block(element,depth) {
    if(depth>32||++count>8192)throw Error('Rich block limit');
    const tag=element.tagName;let kind;
    if(/^H[1-6]$/.test(tag))kind=['heading',Number(tag[1])];
    else switch(tag){
      case 'BLOCKQUOTE':kind=['quote'];break;
      case 'UL':kind=['list',null];break;
      case 'OL':kind=['list',element.getAttribute('start')||'1'];break;
      case 'LI':kind=['item',element.hasAttribute('data-gly-task')?element.getAttribute('data-gly-task')==='true':null];break;
      case 'PRE':kind=['code',element.getAttribute('data-gly-language')||''];break;
      case 'HR':kind=['rule'];break;
      case 'TABLE':kind=['table',(element.getAttribute('data-gly-align')||'').split('').map(v=>({l:'left',c:'center',r:'right'}[v]||'none'))];break;
      case 'TR':kind=[element.hasAttribute('data-gly-table-head')?'head':'row'];break;
      case 'TD':case 'TH':kind=['cell'];break;
      default:kind=['paragraph'];
    }
    const result={id:id(element,s,seen),kind,spans:[],children:[]};if(['paragraph','heading','code','cell'].includes(kind[0]))bindings.set(result.id,element);
    if(['paragraph','heading','code','cell'].includes(kind[0])) {
      for(const child of element.childNodes)inline(child,result.spans,blankMarks(),0);
      if(kind[0]==='code')result.spans.forEach(span=>{if(span.marks)span.marks=blankMarks();});
    } else if(kind[0]!=='rule') {
      let pending=[];
      const flush=()=>{if(!pending.length)return;const first=pending[0],spans=[];pending.forEach(child=>inline(child,spans,blankMarks(),0));pending=[];
        if(spans.length){const identity=id(first,s,seen);bindings.set(identity,element);result.children.push({id:identity,kind:['paragraph'],spans,children:[]});}};
      const children = kind[0]==='table' ? [...element.children].flatMap(child=>['TBODY','THEAD','TFOOT'].includes(child.tagName)?[...child.children]:[child]) : [...element.childNodes];
      for(const child of children){if(child.nodeType===1&&blockTags.has(child.tagName)){flush();result.children.push(block(child,depth+1));}else pending.push(child);}flush();
      if(kind[0]==='table'&&!kind[1].length)kind[1]=Array(result.children[0]?.children.length||1).fill('none');
    }
    return result;
  }
  const children=[];let loose=[];
  const flush=()=>{if(!loose.length)return;const first=loose[0],spans=[];loose.forEach(node=>inline(node,spans,blankMarks(),0));loose=[];
    if(spans.length){const identity=id(first,s,seen);bindings.set(identity,root);children.push({id:identity,kind:['paragraph'],spans,children:[]});}};
  for(const child of root.childNodes){if(child.nodeType===1&&blockTags.has(child.tagName)){flush();children.push(block(child,1));}else loose.push(child);}flush();
  if(!children.length){const identity=id(root,s,seen);bindings.set(identity,root);children.push({id:identity,kind:['paragraph'],spans:[],children:[]});}
  return {document:{root:{id:'0',kind:['root'],spans:[],children}},bindings};
}
function units(node) {
  if(node.nodeType===3)return canonical(node.data).length;
  if(node.nodeType!==1)return 0;
  if(node.tagName==='IMG')return 1;
  if(node.tagName==='BR')return node.hasAttribute('data-gly-rich-empty')?0:1;
  let total=0;for(const child of node.childNodes)total+=units(child);return total;
}
function readSelection(root,model) {
  const selection=root.ownerDocument.getSelection();
  function point(node,offset) {
    let element=node?.nodeType===1?node:node?.parentElement;
    while(element){
      const entry=[...model.bindings].find(([,bound])=>bound===element);
      if(entry){let total=0,found=false;
        function visit(current){if(found)return;if(current===node){if(current.nodeType===3)total+=offset;else for(let i=0;i<Math.min(offset,current.childNodes.length);i++)total+=units(current.childNodes[i]);found=true;return;}
          if(current.nodeType===3||current.nodeType===1&&['IMG','BR'].includes(current.tagName)){total+=units(current);return;}
          for(const child of current.childNodes)visit(child);}
        visit(element);return {block:entry[0],offset:total};}
      if(element===root)break;
      element=element.parentElement;
    }
    const queue=[model.document.root];while(queue.length){const leaf=queue.shift();if(!leaf.children.length&&['paragraph','heading','item','code','cell'].includes(leaf.kind[0]))return {block:leaf.id,offset:0};queue.unshift(...leaf.children);}throw Error('Rich document has no editable leaf');
  }
  return {anchor:point(selection?.anchorNode,selection?.anchorOffset||0),focus:point(selection?.focusNode,selection?.focusOffset||0)};
}
export function readRich(root) {
  const model=readModel(root);return JSON.stringify({document:model.document,selection:readSelection(root,model)});
}
export function selectionRich(root) {const model=readModel(root);return JSON.stringify(readSelection(root,model));}
export function setSelectionRich(root,json) {
  const requested=JSON.parse(json),model=readModel(root),document=root.ownerDocument;
  function locate(point){const element=model.bindings.get(point.block);if(!element)throw Error('Rich selection block gone');let remaining=point.offset,result=null;
    function visit(node){if(result)return;if(node.nodeType===3){if(remaining<=node.data.length){result=[node,remaining];return;}remaining-=node.data.length;return;}
      if(node.nodeType===1&&['IMG','BR'].includes(node.tagName)){const index=[...node.parentNode.childNodes].indexOf(node);if(remaining===0){result=[node.parentNode,index];return;}remaining-=units(node);if(remaining===0)result=[node.parentNode,index+1];return;}
      for(const child of node.childNodes)visit(child);}
    visit(element);if(!result&&remaining===0)result=[element,element.childNodes.length];if(!result)throw Error('Rich selection out of bounds');return result;}
  const anchor=locate(requested.anchor),focus=locate(requested.focus),selection=document.getSelection();
  selection.setBaseAndExtent(anchor[0],anchor[1],focus[0],focus[1]);
}
export function writeRich(root,json) {
  const document=JSON.parse(json);validateRich(document);const s=state(root);let prior=null;
  if(root.childNodes.length){const current=readModel(root);if(JSON.stringify(current.document)===JSON.stringify(document)){s.last=document;return;}
    if(root.contains(root.ownerDocument.getSelection()?.anchorNode))prior=readSelection(root,current);}
  const dom=root.ownerDocument;
  function block(value){const [kind,arg]=value.kind;const tag={root:'div',paragraph:'p',heading:'h'+arg,quote:'blockquote',list:arg===null?'ul':'ol',item:'li',code:'pre',rule:'hr',table:'table',head:'tr',row:'tr',cell:'td'}[kind];
    if(!tag)throw Error('Unknown rich block');const node=dom.createElement(tag);node.dataset.glyRichBlock=value.id;s.ids.set(node,value.id);s.next=s.next>BigInt(value.id)?s.next:BigInt(value.id)+1n;
    if(kind==='list'&&arg!==null)node.setAttribute('start',arg);
    if(kind==='item'&&arg!==null){node.dataset.glyTask=String(arg);const check=dom.createElement('input');check.type='checkbox';check.checked=arg;check.dataset.glyTaskControl='';check.setAttribute('aria-label','Task completed');check.contentEditable='false';node.append(check);}
    if(kind==='code')node.dataset.glyLanguage=arg;
    if(kind==='head')node.dataset.glyTableHead='';
    if(kind==='table')node.dataset.glyAlign=arg.map(v=>({left:'l',center:'c',right:'r'}[v]||'n')).join('');
    for(const span of value.spans){
      if(span.text!==undefined){let target=node;const m=span.marks;
        for(const [enabled,tag]of [[m.link,'a'],[m.bold,'strong'],[m.italic,'em'],[m.strike,'s'],[m.code,'code']])if(enabled){const next=dom.createElement(tag);if(tag==='a'){next.setAttribute('href',m.link);next.setAttribute('rel','noreferrer noopener');}target.append(next);target=next;}
        target.append(dom.createTextNode(span.text));
      }else if(span.image){const image=dom.createElement('img');image.setAttribute('src',span.image.source);image.setAttribute('alt',span.image.alternative);image.setAttribute('title',span.image.title);image.contentEditable='false';node.append(image);}
      else if(span.break)node.append(dom.createElement('br'));
    }
    for(const child of value.children)node.append(block(child));
    if(kind==='table')for(const row of node.children)for(let i=0;i<row.children.length;i++)row.children[i].style.textAlign=arg[i]==='none'?'':arg[i];
    if(['paragraph','heading','code','cell'].includes(kind)&&!value.spans.length){const br=dom.createElement('br');br.dataset.glyRichEmpty='';node.append(br);}return node;
  }
  const fragment=dom.createDocumentFragment();for(const child of document.root.children)fragment.append(block(child));root.replaceChildren(fragment);s.last=document;
  if(prior){try{setSelectionRich(root,JSON.stringify(prior));}catch{ /* removed text has no surviving selection */ }}
}
export function pasteRich(root,event) {
  if(root.contentEditable!=='true')return false;
  const data=event.clipboardData||event.dataTransfer;if(!data)return false;
  event.preventDefault();const selection=root.ownerDocument.getSelection();if(!selection.rangeCount||!root.contains(selection.anchorNode))return false;
  const text=canonical(data.getData('text/plain'));if(text.length>1048576)throw Error('Paste exceeds rich text budget');
  const range=selection.getRangeAt(0);range.deleteContents();const node=root.ownerDocument.createTextNode(text);range.insertNode(node);range.setStartAfter(node);range.collapse(true);selection.removeAllRanges();selection.addRange(range);return true;
}

export function taskRich(root,event) {
 const check=event.target;if(!check?.hasAttribute?.('data-gly-task-control')||!root.contains(check))return false;
 event.preventDefault();if(root.contentEditable!=='true')return false;
 const item=check.closest('li[data-gly-task]');if(!item)return false;
 const checked=item.dataset.glyTask!=='true';item.dataset.glyTask=String(checked);
 // Cancelling a checkbox click restores its former checked state after capture.
 queueMicrotask(()=>{if(root.contains(check))check.checked=checked;});return true;
}
function markedText(root,text,marks) {
 const dom=root.ownerDocument,fragment=dom.createDocumentFragment();let target=fragment;
 for(const[enabled,tag]of [[marks.link,'a'],[marks.bold,'strong'],[marks.italic,'em'],[marks.strike,'s'],[marks.code,'code']])if(enabled){const next=dom.createElement(tag);if(tag==='a'){next.setAttribute('href',marks.link);next.setAttribute('rel','noreferrer noopener');}target.append(next);target=next;}
 const node=dom.createTextNode(text);target.append(node);return {fragment,node};
}
export function setMarksRich(root,json) {
 const marks=JSON.parse(json),selection=JSON.parse(selectionRich(root));
 if(JSON.stringify(selection.anchor)!==JSON.stringify(selection.focus))throw Error('Typing marks require a collapsed selection');
 validateRich({root:{id:'0',kind:['root'],spans:[],children:[{id:'1',kind:['paragraph'],spans:[{text:'x',marks}],children:[]}]}});
 state(root).typing={marks,selection};
}
function typing(root) {
 const s=state(root);if(!s.typing)return null;
 const selection=JSON.parse(selectionRich(root));if(JSON.stringify(selection)!==JSON.stringify(s.typing.selection)){s.typing=null;return null;}return s.typing;
}
export function beforeInputRich(root,event) {
 if(root.contentEditable!=='true'||event.isComposing||event.inputType!=='insertText'||typeof event.data!=='string')return false;
 const pending=typing(root);if(!pending)return false;event.preventDefault();
 const selection=root.ownerDocument.getSelection();if(!selection.rangeCount)return false;
 const range=selection.getRangeAt(0),inserted=markedText(root,canonical(event.data),pending.marks);range.deleteContents();range.insertNode(inserted.fragment);
 range.setStart(inserted.node,inserted.node.length);range.collapse(true);selection.removeAllRanges();selection.addRange(range);
 pending.selection=JSON.parse(selectionRich(root));return true;
}
export function compositionRich(root) {
 const pending=typing(root),s=state(root);s.composition=null;
 if(!pending||root.contentEditable!=='true')return;
 // Chromium has already captured its replacement Range before this callback.
 // Inserting an empty style span or moving Selection here does not retarget
 // the native IME; it writes outside that span. Preserve its DOM throughout
 // preedit and apply the finite typing marks only to the verified commit range.
 s.composition={marks:{...pending.marks},point:{...pending.selection.anchor}};
}
export function finishCompositionRich(root,event) {
 const s=state(root),pending=s.composition;s.composition=null;
 if(!pending||root.contentEditable!=='true')return;
 const text=typeof event.data==='string'?canonical(event.data):'';
 if(!text)return;
 const model=readModel(root),selection=readSelection(root,model),start=pending.point.offset,end=start+text.length;
 if(selection.anchor.block!==pending.point.block||selection.focus.block!==pending.point.block||selection.anchor.offset!==end||selection.focus.offset!==end)return;
 const stack=[model.document.root];let block=null;
 while(stack.length){const next=stack.pop();if(next.id===pending.point.block){block=next;break;}stack.push(...next.children);}
 if(!block||block.kind[0]==='code')return;
 const value=block.spans.map(span=>span.text??(span.break?'\n':'\ufffc')).join('');
 // Selection relocation, cancellation, program replacement and cross-block
 // changes must not apply stale marks to unrelated text.
 if(value.slice(start,end)!==text)return;
 const spans=[];let offset=0;
 for(const span of block.spans){const length=span.text?.length??1,next=offset+length;
  if(span.text===undefined||next<=start||offset>=end)spans.push(span);
  else {const from=Math.max(0,start-offset),to=Math.min(length,end-offset);
   spanText(spans,span.text.slice(0,from),span.marks);
   spanText(spans,span.text.slice(from,to),pending.marks);
   spanText(spans,span.text.slice(to),span.marks);
  }offset=next;
 }
 block.spans=spans;writeRich(root,JSON.stringify(model.document));
 setSelectionRich(root,JSON.stringify(selection));s.typing={marks:pending.marks,selection};
}
