function gloryWry(config) {
  "use strict";
  if (window !== window.top) return;
  const entries = new Map(), owners = new WeakMap();
  let root, stopped = false, sequence = 0n;
  const post = payload => {
    if (stopped) return;
    const message = JSON.stringify({v: config.v, session: config.session, ...payload});
    if (new TextEncoder().encode(message).length > 1048576) {
      window.ipc.postMessage(JSON.stringify({v:config.v,session:config.session,type:"fault",message:"IPC byte budget"}));
      stopped = true;
      return;
    }
    window.ipc.postMessage(message);
  };
  const fault = error => { post({type:"fault",message:String(error).slice(0,2048)}); stopped = true; };
  window.addEventListener("error",e=>fault(e.message || "WebView script error"));
  window.addEventListener("unhandledrejection",e=>fault(e.reason));
  const get = id => { const e=entries.get(id); if (!e) throw Error("unknown node " + id); return e; };
  const main = e => e.parts.get("main");
  const control = e => e.parts.get(e.control || "main");
  const alive = e => !!(e.start.isConnected && !main(e)?.closest?.("[inert],[data-gly-hidden]"));
  // Only one text proposal per editor crosses IPC at a time. Native edits may
  // continue locally; retain a dirty bit, not a history of values or revisions.
  const editing = new Set(), deferred = [];
  let deferredBytes=0;
  const event = (e,value) => {
    if(value.kind==="value") {
      if(e.proposal) { e.dirty=true; return; }
      e.proposal={value:value.value,composing:value.composing};
      e.dirty=false; editing.add(e);
    } else if(editing.size && ["activate","commit","keyboard"].includes(value.kind)) {
      const bytes=new TextEncoder().encode(JSON.stringify(value)).length;
      if(deferred.length>=128 || deferredBytes+bytes>1048576) throw Error("pending input action budget");
      deferredBytes+=bytes; deferred.push({e,value,bytes}); return;
    }
    post({type:"event",node:e.id,event:value});
  };
  function flushActions() {
    while(!editing.size && deferred.length) {
      const {e,value,bytes}=deferred.shift();
      deferredBytes-=bytes;
      if(!entries.has(e.id) || !alive(e)) continue;
      if(value.kind==="commit") {
        if(readValue(e)!==value.value) continue;
        value.revision=e.revision;
      }
      event(e,value);
    }
  }
  function inputDone(command) {
    const e=entries.get(command.id);
    if(!e || !e.proposal) return;
    const proposal=e.proposal, dirty=e.dirty;
    e.proposal=null; e.dirty=false; editing.delete(e);
    if(command.retired) return;
    if(command.reset) { e.requested=command.revision; property(e,e.control,"value",command.value); return; }
    if(e.pending!==null && e.composing) return;
    e.revision=command.revision;
    if(dirty && alive(e) && e.interests.has("ValueChanged")) {
      const value=readValue(e);
      if(value!==proposal.value || e.composing!==proposal.composing)
        event(e,{kind:"value",value,revision:e.revision,composing:e.composing});
    }
  }
  const observed = (e,kind) => {
    for (let current=e;current;current=current.parent ? entries.get(current.parent) : null)
      if (current.interests.has(kind)) return true;
    return false;
  };
  const origin = node => {
    for (let n=node;n && n!==root;n=n.parentNode) { const e=owners.get(n); if(e) return e; }
    return null;
  };
  const viewport = element => ({viewport_width:element.clientWidth,viewport_height:element.clientHeight});
  const scroll = e => {
    const element=main(e);
    if (e.interests.has("Scroll") && alive(e))
      event(e,{kind:"scroll",x:element.scrollLeft,y:element.scrollTop,...viewport(element),ack:e.ack});
  };
  const resize = (e,box) => {
    const element=main(e);
    if (!alive(e) || !e.interests.has("Resize")) return;
    const rect=box || element.getBoundingClientRect();
    event(e,{kind:"resize",width:rect.width,height:rect.height,...viewport(element)});
  };
  const observer = new ResizeObserver(records => {
    for (const record of records) {
      const e=owners.get(record.target);
      if(e) {
        const box=record.borderBoxSize?.[0];
        const vertical=getComputedStyle(record.target).writingMode.startsWith("vertical");
        resize(e,box ? {width:vertical?box.blockSize:box.inlineSize,height:vertical?box.inlineSize:box.blockSize}:null);
      }
    }
  });
  const canonical = value => value.replace(/\r\n?/g,"\n").replace(/\0/g,"\ufffd");
  function readValue(e) {
    const input=control(e),raw=input.value,value=canonical(raw);
    if(raw!==value) {
      const selection=typeof input.selectionStart==="number" ? [canonical(raw.slice(0,input.selectionStart)).length,canonical(raw.slice(0,input.selectionEnd)).length,input.selectionDirection] : null;
      input.value=value;
      if(selection) input.setSelectionRange(...selection);
    }
    return value;
  }
  function property(e,part,name,value) {
    const element=e.parts.get(part);
    if(name==="value" && e.edit==="Text") {
      value=canonical(value);
      // An echo acknowledges the submitted snapshot. It must not replace newer
      // native typing that is waiting for its own proposal turn.
      if(e.proposal && e.dirty && value===e.proposal.value) {
        e.revision=e.requested; e.pending=null; return;
      }
      if(e.proposal && value!==e.proposal.value) e.dirty=false;
      if(e.composing && readValue(e)!==value) { e.pending=value; return; }
      if(readValue(e)!==value) element.value=value;
      e.revision=e.requested;
      e.pending=null;
    } else if(element[name]!==value) element[name]=value;
  }
  function attribute(e,part,name,value) {
    const element=e.parts.get(part);
    if(value===null) element.removeAttribute(name); else element.setAttribute(name,value);
  }
  function create(command) {
    if(entries.has(command.id)) throw Error("duplicate node");
    const shape=command.shape, park=document.createDocumentFragment();
    const e={id:command.id,parts:new Map(),children:shape.children,control:shape.control,
      edit:shape.edit,prevent:shape.prevent,revision:shape.revision,requested:shape.revision,
      composing:false,pending:null,proposal:null,dirty:false,parent:null,interests:new Set(),park,ack:null};
    for(const piece of shape.pieces) {
      const kind=piece.kind;
      const element=kind.tag ? document.createElement(kind.tag) : document.createTextNode(kind.text);
      e.parts.set(piece.part,element); owners.set(element,e);
      if(kind.tag) for(const [name,value] of Object.entries(kind.attributes)) element.setAttribute(name,value);
      (piece.parent ? e.parts.get(piece.parent) : park).appendChild(element);
    }
    if(!shape.pieces.length) {
      e.start=document.createComment("glory-group"); e.end=document.createComment("/glory-group");
      park.append(e.start,e.end);
    } else { e.start=park.firstChild; e.end=park.lastChild; }
    entries.set(e.id,e);
    for(const piece of shape.pieces) if(piece.kind.tag)
      for(const [name,value] of Object.entries(piece.kind.properties)) property(e,piece.part,name,value);
    const element=main(e);
    if(element?.hasAttribute("data-gly-virtual")) element.setAttribute("data-gly-live","");
    if(element?.tagName==="IMG") {
      const status=loaded => {
        if(alive(e) && e.interests.has("ImageStatus")) event(e,{kind:"image",revision:e.requested,loaded});
      };
      element.addEventListener("load",()=>status(true)); element.addEventListener("error",()=>status(false));
    }
  }
  function extract(e) {
    const fragment=document.createDocumentFragment();
    let current=e.start;
    while(current) { const next=current.nextSibling; fragment.appendChild(current); if(current===e.end) break; current=next; }
    return fragment;
  }
  function place(command) {
    const e=get(command.id), parent=command.parent ? get(command.parent) : null;
    const container=parent ? (parent.children ? parent.parts.get(parent.children) : parent.end.parentNode) : root;
    const before=command.before ? get(command.before).start : (parent && !parent.children ? parent.end : null);
    if(e.start===e.end && e.start.isConnected && container.isConnected && container.moveBefore)
      container.moveBefore(e.start,before);
    else {
      const focused=document.activeElement;
      const focusEntry=origin(focused);
      const selection=focused && typeof focused.selectionStart==="number" ? [focused.selectionStart,focused.selectionEnd,focused.selectionDirection] : null;
      let contained=false;
      for(let n=e.start;n;n=n.nextSibling) { if(n===focused || n.contains?.(focused)) contained=true; if(n===e.end) break; }
      container.insertBefore(extract(e),before);
      if(contained && focusEntry && focused.isConnected && document.activeElement!==focused) {
        focused.focus({preventScroll:true});
        if(selection) focused.setSelectionRange(...selection);
      }
    }
    e.parent=command.parent;
  }
  function contains(e,node) {
    for(let current=e.start;current;current=current.nextSibling) {
      if(current===node || current.contains?.(node)) return true;
      if(current===e.end) break;
    }
    return false;
  }
  function suspend(e) {
    const focused=origin(document.activeElement);
    if(focused && contains(e,document.activeElement)) {
      if(focused.composing) {
        focused.composing=false;
        if(focused.pending!==null) property(focused,focused.control,"value",focused.pending);
        event(focused,{kind:"composition",active:false});
      }
      control(focused).blur();
    }
    e.park.appendChild(extract(e));
  }
  function query(command) {
    let value=null,error=null;
    try {
      const e=get(command.id),element=main(e),input=control(e);
      if(!alive(e)) throw Error("inactive");
      switch(command.kind) {
        case "bounds": {const r=element.getBoundingClientRect();value={x:r.x,y:r.y,width:r.width,height:r.height};break;}
        case "scroll": value={x:element.scrollLeft,y:element.scrollTop};break;
        case "scroll_metrics": value={x:element.scrollLeft,y:element.scrollTop,...viewport(element),content_width:element.scrollWidth,content_height:element.scrollHeight};break;
        case "focus": value=document.activeElement===input;break;
        case "selection":
          if(typeof input.selectionStart!=="number") throw Error("selection unavailable");
          value={start:input.selectionStart,end:input.selectionEnd};break;
        default: throw Error("unknown query");
      }
    } catch(reason) {error=String(reason);}
    post({type:"response",token:command.token,value,error});
  }
  function action(command) {
    const e=get(command.id),element=main(e),input=control(e),a=command.action;
    if(!alive(e)) return;
    switch(a.kind) {
      case "focus": input.focus({preventScroll:true});break;
      case "scroll":
        element.scrollTo({left:a.x,top:a.y,behavior:"instant"});
        e.ack={requested_x:a.x,requested_y:a.y,x:element.scrollLeft,y:element.scrollTop};scroll(e);break;
      case "reveal": element.scrollIntoView({block:"nearest",inline:"nearest",behavior:"instant"});break;
      case "selection":
        if(e.revision===a.revision && !e.composing) input.setSelectionRange(a.start,a.end,a.direction);
        break;
      default: throw Error("unknown action");
    }
  }
  const media=matchMedia("(prefers-color-scheme:dark)");
  const color = () => { for(const e of entries.values()) if(e.interests.has("ColorSchemeChanged") && alive(e)) event(e,{kind:"color_scheme",dark:media.matches}); };
  media.addEventListener("change",color);
  function apply(command) {
    if(command.op==="input_done") return inputDone(command);
    if(command.op==="create") return create(command);
    if(command.op==="query") return query(command);
    if(command.op==="action") return action(command);
    if(command.op==="place") return place(command);
    if(command.op==="css") { document.getElementById("glory-author").textContent=command.value; return; }
    const e=get(command.id);
    switch(command.op) {
      case "patch":
        for(const edit of command.edits) switch(edit.kind) {
          case "revision": e.requested=edit.value; if(e.edit!=="Text") e.revision=edit.value;break;
          case "attribute": attribute(e,edit.part,edit.name,edit.value);break;
          case "property": property(e,edit.part,edit.name,edit.value);break;
          case "text": {
            const target=e.parts.get(edit.part);
            if(target.nodeType!==Node.TEXT_NODE) throw Error("text patch cannot replace children");
            target.data=edit.value;break;
          }
          default: throw Error("unknown edit");
        }
        break;
      case "listen": {
        if(command.enabled) e.interests.add(command.kind); else e.interests.delete(command.kind);
        const element=main(e);
        if(command.kind==="Resize") { if(command.enabled) {observer.observe(element,{box:"border-box"});resize(e);} else observer.unobserve(element); }
        if(command.kind==="Scroll" && command.enabled) scroll(e);
        if(command.kind==="ColorSchemeChanged" && command.enabled && alive(e)) event(e,{kind:"color_scheme",dark:media.matches});
        break;
      }
      case "suspend": suspend(e);break;
      case "dispose":
        extract(e);
        for(const id of command.subtree) {
          const old=entries.get(id);
          if(old) { if(main(old)) observer.unobserve(main(old)); entries.delete(id); editing.delete(old); old.proposal=null; old.dirty=false; old.park.replaceChildren(); }
        }
        break;
      default: throw Error("unknown operation");
    }
  }
  Object.defineProperty(window,"__gloryWryReceive",{value:batch=> {
    if(stopped) return;
    try {
      if(batch.v!==config.v || batch.session!==config.session || BigInt(batch.sequence)!==sequence+1n) throw Error("batch identity/order");
      sequence=BigInt(batch.sequence);
      for(const command of batch.commands) apply(command);
      flushActions();
      post({type:"ack",sequence:batch.sequence});
    } catch(error) {fault(error);}
  }});
  function receive(domEvent) {
    const e=origin(domEvent.target);
    if(!e || !alive(e)) return;
    const input=control(e), element=main(e);
    const disabled=!input || input.disabled || element.getAttribute("aria-disabled")==="true";
    switch(domEvent.type) {
      case "click":
        if(disabled || input.readOnly && e.edit==="Check") {domEvent.preventDefault();return;}
        if(e.prevent || element.tagName==="A") domEvent.preventDefault();
        if(observed(e,"Activate")) event(e,{kind:"activate"});break;
      case "input":
        if(e.edit!=="Text" || disabled || input.readOnly) return;
        if(!e.composing && domEvent.isComposing) event(e,{kind:"composition",active:true});
        e.composing ||= !!domEvent.isComposing;
        if(e.interests.has("ValueChanged")) event(e,{kind:"value",value:readValue(e),revision:e.revision,composing:e.composing});break;
      case "change":
        if(e.edit==="Check" && !disabled && !input.readOnly && e.interests.has("CheckedChanged")) event(e,{kind:"checked",value:input.checked});break;
      case "compositionstart":
        if(e.edit!=="Text") return;
        e.composing=true;
        event(e,{kind:"composition",active:true});break;
      case "compositionend":
        if(e.edit!=="Text") return;
        e.composing=false;
        if(e.pending!==null) property(e,e.control,"value",e.pending);
        event(e,{kind:"composition",active:false});
        if(e.interests.has("ValueChanged")) event(e,{kind:"value",value:readValue(e),revision:e.revision,composing:false});break;
      case "focusin": case "focusout":
        if(observed(e,"FocusWithin") || e.interests.has("FocusChanged")) event(e,{kind:"focus",focused:domEvent.type==="focusin"});break;
      case "scroll": if(domEvent.target===element) scroll(e);break;
      case "keydown": {
        if(domEvent.isComposing || e.composing || domEvent.repeat || domEvent.altKey) return;
        const key=domEvent.key,editable=e.edit==="Text";
        if(key==="Enter" && editable && input.tagName!=="TEXTAREA" && !disabled && !input.readOnly && e.interests.has("Commit")) {
          domEvent.preventDefault();event(e,{kind:"commit",value:readValue(e),revision:e.revision});
        }
        if(!editable && !domEvent.ctrlKey && !domEvent.metaKey && !domEvent.shiftKey && observed(e,"Navigate")) {
          const command={ArrowUp:"previous",ArrowDown:"next",Home:"first",End:"last",PageUp:"page_previous",PageDown:"page_next"}[key];
          if(command) {domEvent.preventDefault();event(e,{kind:"navigate",command});return;}
        }
        const chord=domEvent.ctrlKey || domEvent.metaKey;
        const command=key==="Escape"?"dismiss":editable?(key==="Enter"&&!domEvent.shiftKey?"submit":null):
          chord&&!domEvent.shiftKey&&key.toLowerCase()==="k"?"palette":chord&&!domEvent.shiftKey&&key.toLowerCase()==="f"?"search":
          key==="?"||chord&&key==="/"||domEvent.shiftKey&&key==="/"?"help":null;
        if(command && observed(e,"Keyboard")) {domEvent.preventDefault();event(e,{kind:"keyboard",command});}
        break;
      }
    }
  }
  function start() {
    root=document.getElementById("glory-root");
    if(!root) {fault("missing Glory mount");return;}
    for(const name of ["click","input","change","compositionstart","compositionend","focusin","focusout","scroll","keydown"])
      root.addEventListener(name,receive,true);
    post({type:"ready"});
  }
  if(document.readyState==="loading") document.addEventListener("DOMContentLoaded",start,{once:true}); else start();
}
