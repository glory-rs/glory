function gloryWry(config) {
  "use strict";
  if (window !== window.top) return;
  const entries = new Map(), owners = new WeakMap();
  let root, stopped = false, sequence = 0n;
  const post = payload => {
    if (stopped) return;
    const message = JSON.stringify({v: config.v, session: config.session, ...payload});
    if (new TextEncoder().encode(message).length > config.maxIpcBytes) {
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
    } else if(editing.size && ["activate","commit","keyboard","focus"].includes(value.kind)) {
      const bytes=new TextEncoder().encode(JSON.stringify(value)).length;
      if(deferred.length>=128 || deferredBytes+bytes>config.maxDeferredBytes) throw Error("pending input action budget");
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
  function selectOptions(element, config) {
    function appendOption(parent, entry) {
      const option = document.createElement("option");
      option.dataset.glyOption = entry.key;
      option.textContent = entry.label;
      option.value = entry.value;
      option.disabled = entry.disabled;
      parent.append(option);
    }
    if (config.placeholder !== null) {
      const placeholder = document.createElement("option");
      placeholder.dataset.glyPlaceholder = "";
      placeholder.value = "";
      placeholder.textContent = config.placeholder;
      placeholder.hidden = placeholder.disabled = true;
      element.append(placeholder);
    }
    for (const entry of config.entries) {
      if (!entry.options) { appendOption(element, entry); continue; }
      const group = document.createElement("optgroup");
      group.label = entry.label;
      group.disabled = entry.disabled;
      for (const option of entry.options) appendOption(group, option);
      element.append(group);
    }
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
    } else if(name==="selectedKeys" && e.edit==="Select") {
      const keys = new Set(value);
      for (const option of element.options) {
        const selected = option.hasAttribute("data-gly-option") ? keys.has(option.dataset.glyOption) : !keys.size;
        option.defaultSelected = selected;
        option.selected = selected;
      }
    } else if(element[name]!==value) element[name]=value;
  }
  function attribute(e,part,name,value) {
    const element=e.parts.get(part);
    if(value===null) element.removeAttribute(name); else element.setAttribute(name,value);
    if(name==="data-gly-drag-capture" && value===null && drag?.e===e)finishDrag("cancel",drag.x,drag.y);
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
    if(shape.options) selectOptions(control(e),shape.options);
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
    for(const child of entries.values()) if(child.mediaSuspended && alive(child) && contains(e,child.start)) {installMedia(child);child.mediaSuspended=false;}
  }
  function contains(e,node) {
    for(let current=e.start;current;current=current.nextSibling) {
      if(current===node || current.contains?.(node)) return true;
      if(current===e.end) break;
    }
    return false;
  }
  function stopMedia(e,remember=false) {
    if(e.localMedia) {
      e.mediaPosition=remember && Number.isFinite(e.localMedia.currentTime) ? e.localMedia.currentTime : 0;
      e.mediaCleanup?.(); e.mediaCleanup=null; e.localMedia.pause?.(); e.localMedia.removeAttribute("src"); e.localMedia.load?.(); e.mediaSurface?.remove(); e.mediaSurface=null; e.localMedia.remove(); e.localMedia=null;
    }
  }
  function installMedia(e) {
    const value=e.mediaConfig;if(!value)return;
    const player=document.createElement(value.kind);
    player.controls=true; player.preload="metadata"; player.autoplay=false;
    player.setAttribute("playsinline",""); player.setAttribute("aria-label",value.title);
    player.style.width="100%"; player.style.maxHeight="480px";
    if(e.mediaPosition>0) player.addEventListener('loadedmetadata',()=>{if(e.localMedia===player && Number.isFinite(player.duration))player.currentTime=Math.min(e.mediaPosition,Math.max(0,player.duration-.1));},{once:true});
    if(value.kind==='img') {
      const container=document.createElement('div');container.className='glory-image-viewer';
      const toolbar=document.createElement('div');toolbar.style.cssText='display:flex;gap:.5em;padding:.5em';
      const surface=document.createElement('div');surface.style.cssText='overflow:auto;max-height:70vh;min-height:120px;position:relative;touch-action:none;cursor:grab';
      const canvas=document.createElement('div');canvas.style.position='relative';
      player.alt=value.title;player.draggable=false;player.style.cssText='display:block;position:absolute;left:50%;top:50%;max-width:none;max-height:none;transform-origin:center';
      let zoom=1,rotation=0;const update=()=>{
        const width=player.naturalWidth,height=player.naturalHeight;if(!width||!height)return;
        const rotated=rotation%180!==0;const rw=rotated?height:width,rh=rotated?width:height;
        const scale=Math.min((surface.clientWidth||640)/rw,Math.max(120,window.innerHeight*.7)/rh)*zoom;
        canvas.style.width=`${rw*scale}px`;canvas.style.height=`${rh*scale}px`;
        player.style.width=`${width*scale}px`;player.style.height=`${height*scale}px`;player.style.transform=`translate(-50%,-50%) rotate(${rotation}deg)`;
        player.dataset.zoom=String(zoom);player.dataset.rotation=String(rotation);
      };
      const control=(label,symbol,run)=>{const button=document.createElement('button');button.type='button';button.textContent=symbol;button.setAttribute('aria-label',label);button.title=label;button.addEventListener('click',run);toolbar.appendChild(button);};
      const change=scale=>{zoom=Math.max(.1,Math.min(8,zoom*scale));update();};
      control('Zoom in','+',()=>change(1.2));control('Zoom out','−',()=>change(1/1.2));control('Rotate image','↻',()=>{rotation=(rotation+90)%360;update();});
      const fit=()=>{zoom=1;rotation=0;update();surface.scrollTo(0,0);};control('Fit image','⊡',fit);
      surface.addEventListener('wheel',event=>{event.preventDefault();change(event.deltaY<0?1.2:1/1.2);},{passive:false});
      surface.addEventListener('dblclick',fit);
      const pointers=new Map();let pinch=0;
      surface.addEventListener('pointerdown',event=>{surface.setPointerCapture(event.pointerId);pointers.set(event.pointerId,{x:event.clientX,y:event.clientY});pinch=0;});
      surface.addEventListener('pointermove',event=>{const before=pointers.get(event.pointerId);if(!before)return;pointers.set(event.pointerId,{x:event.clientX,y:event.clientY});if(pointers.size===2){const [a,b]=[...pointers.values()];const distance=Math.hypot(a.x-b.x,a.y-b.y);if(pinch>0)change(distance/pinch);pinch=distance;}else{surface.scrollLeft-=event.clientX-before.x;surface.scrollTop-=event.clientY-before.y;}});
      for(const kind of ['pointerup','pointercancel','lostpointercapture'])surface.addEventListener(kind,event=>{pointers.delete(event.pointerId);pinch=0;});
      player.addEventListener('load',update);
      // Never mutate observed geometry during ResizeObserver delivery. Only
      // width or actual viewport changes affect fit; image height changes do not
      // recursively schedule themselves.
      let frame=0,lastWidth=-1;
      const schedule=()=>{if(!frame)frame=requestAnimationFrame(()=>{frame=0;if(entries.get(e.id)===e&&e.localMedia===player)update();});};
      const observer=new ResizeObserver(()=>{const width=surface.clientWidth;if(width!==lastWidth){lastWidth=width;schedule();}});
      observer.observe(surface);window.addEventListener('resize',schedule);
      e.mediaCleanup=()=>{observer.disconnect();window.removeEventListener('resize',schedule);if(frame)cancelAnimationFrame(frame);};
      player.src=value.path;canvas.appendChild(player);surface.appendChild(canvas);container.append(toolbar,surface);main(e).appendChild(container);e.mediaSurface=container;e.localMedia=player;
    } else {player.src=value.path;main(e).appendChild(player);e.localMedia=player;}
  }
  function suspend(e) {
    for(const child of entries.values()) if(child.localMedia && contains(e,child.localMedia)) {stopMedia(child,true);child.mediaSuspended=true;}
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
  let clipboardPending=0;
  async function copyText(e,a) {
    const complete=outcome=>{if(entries.get(e.id)===e)event(e,{kind:'clipboard',request:a.request,revision:a.revision,outcome});};
    if(!config.textClipboard||!alive(e)||control(e).disabled||e.clipboardRevision!==a.revision){complete('unavailable');return;}
    if(clipboardPending>=16||new TextEncoder().encode(a.text).length>65536){complete('failed');return;}
    clipboardPending++;
    try {
      // A temporary plain-text editor avoids HTML interpretation. Restore the
      // exact original input selection, including UTF-16 surrogate boundaries.
      const active=document.activeElement;const selection=active&&typeof active.selectionStart==='number'?{start:active.selectionStart,end:active.selectionEnd,direction:active.selectionDirection}:null;
      const selected=window.getSelection();const ranges=selected?[...Array(selected.rangeCount)].map((_,i)=>selected.getRangeAt(i).cloneRange()):[];
      const editor=document.createElement('textarea');editor.value=a.text;editor.readOnly=true;editor.style.cssText='position:fixed;left:-10000px;top:0';document.body.appendChild(editor);
      let written=false;
      try {editor.select();written=document.execCommand('copy');}catch(_){}finally{editor.value='';editor.remove();if(active?.isConnected){active.focus({preventScroll:true});if(selection)active.setSelectionRange(selection.start,selection.end,selection.direction);}if(!selection&&selected){selected.removeAllRanges();for(const range of ranges)if(range.commonAncestorContainer.isConnected)selected.addRange(range);}}
      if(written){complete('written');return;}
      if(!alive(e)||e.clipboardRevision!==a.revision){complete('failed');return;}
      if(!navigator.clipboard?.writeText){complete('unavailable');return;}
      try{await navigator.clipboard.writeText(a.text);complete('written');}catch(error){complete(error?.name==='NotAllowedError'?'denied':'failed');}
    }finally{clipboardPending--;}
  }
  function action(command) {
    const e=get(command.id),element=main(e),input=control(e),a=command.action;
    if(a.kind==='copy_text'){copyText(e,a).catch(fault);return;}
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
      case "clipboard_source": e.clipboardRevision=command.revision;break;
      case "local_media": {
        stopMedia(e);
        e.mediaConfig=command.value;e.mediaPosition=0;e.mediaSuspended=false;
        if(alive(e)) installMedia(e);else e.mediaSuspended=!!command.value;
        break;
      }
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
          if(old) { if(drag?.e===old)finishDrag("cancel",drag.x,drag.y); stopMedia(old); if(main(old)) observer.unobserve(main(old)); entries.delete(id); editing.delete(old); old.proposal=null; old.dirty=false; old.park.replaceChildren(); }
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
      syncContextLayers();
      flushActions();
      post({type:"ack",sequence:batch.sequence});
    } catch(error) {fault(error);}
  }});
  // Pointer capture is host-owned. Only finite semantic observations leave the
  // interpreter; applications decide what a completed drag means.
  let drag=null, suppressClick=false;
  const dragBlocked=e=>!alive(e) || main(e)?.closest("[hidden],[inert],[data-gly-hidden],[aria-disabled=true],[aria-readonly=true]");
  function dragObserver(target) {
    let entry = origin(target);
    while (entry && !entry.interests.has("Drag")) {
      entry = entry.parent ? entries.get(entry.parent) : null;
    }
    return entry;
  }

  const thresholdCapture = capture => capture === "threshold" || capture === "threshold-preview";

  function hitDragTarget(x, y) {
    let entry = origin(document.elementFromPoint(x, y));
    while (entry) {
      const capture = main(entry)?.getAttribute("data-gly-drag-capture");
      if (entry.interests.has("Drag") && !dragBlocked(entry)
          && (!capture || thresholdCapture(capture))) return entry;
      entry = entry.parent ? entries.get(entry.parent) : null;
    }
    return null;
  }

  function createDragPreview(source) {
    const label = source.getAttribute("aria-label");
    if (!label) return null;
    const preview = document.createElement("div");
    preview.setAttribute("data-gly-drag-preview", "");
    preview.setAttribute("aria-hidden", "true");
    preview.textContent = label;
    preview.style.visibility = "hidden";
    const style = getComputedStyle(source);
    for (const property of ["font", "color-scheme", "--gly-drag-preview-bg",
                             "--gly-drag-preview-text", "--gly-drag-preview-line"]) {
      const value = style.getPropertyValue(property);
      if (value) preview.style.setProperty(property, value);
    }
    document.body.appendChild(preview);
    return preview;
  }

  function placeDragPreview(session) {
    const preview = session.preview;
    if (!preview) return;
    const x = Math.max(8, Math.min(session.x + 14, innerWidth - preview.offsetWidth - 8));
    const y = Math.max(8, Math.min(session.y + 16, innerHeight - preview.offsetHeight - 8));
    const position = "translate(" + x + "px," + y + "px)";
    if (session.previewPosition !== position) {
      preview.style.transform = position;
      preview.style.removeProperty("visibility");
      session.previewPosition = position;
    }
  }

  function finishDrag(phase,x,y) {
    const previous=drag;drag=null;
    if (!previous?.started) return;
    previous.preview?.remove();
    if (entries.get(previous.e.id) === previous.e) {
      const targeted = phase === "drop" && previous.threshold;
      const target = targeted ? hitDragTarget(x, y) : previous.e;
      if (!target) event(previous.e, {kind:"drag",phase:"cancel",x,y});
      else if (targeted) event(target, {kind:"drag",phase,x,y,from:previous.e.id});
      else event(target, {kind:"drag",phase,x,y});
    }
    try{previous.element.releasePointerCapture(previous.pointer);}catch(_){}
  }
  function dragScroll(session) {
    if (drag !== session || !session.started) return;
    if(dragBlocked(drag.e)){finishDrag("cancel",drag.x,drag.y);return;}
    placeDragPreview(drag);
    let changed=false, vertical=false, horizontal=false;
    for(let element=document.elementFromPoint(drag.x,drag.y);element && element!==root;element=element.parentElement) {
      const rect=element.getBoundingClientRect(),style=getComputedStyle(element);
      const delta=(point,start,end)=>point<start+28?-10:point>end-28?10:0;
      if(!vertical && /(auto|scroll)/.test(style.overflowY) && element.scrollHeight>element.clientHeight) {
        const old=element.scrollTop;element.scrollTop+=delta(drag.y,rect.top,rect.bottom);
        vertical=element.scrollTop!==old;changed||=vertical;
      }
      if(!horizontal && /(auto|scroll)/.test(style.overflowX) && element.scrollWidth>element.clientWidth) {
        const old=element.scrollLeft;element.scrollLeft+=delta(drag.x,rect.left,rect.right);
        horizontal=element.scrollLeft!==old;changed||=horizontal;
      }
    }
    if(changed)event(drag.e,{kind:"drag",phase:"move",x:drag.x,y:drag.y});
    requestAnimationFrame(() => dragScroll(session));
  }
  function startDrag(raw, session) {
    try { session.element.setPointerCapture(raw.pointerId); }
    catch (_) { drag = null; return false; }
    session.started = true;
    if (session.showPreview) session.preview = createDragPreview(session.element);
    raw.preventDefault();
    if (session.focus) session.element.focus({preventScroll:true});
    event(session.e,{kind:"drag",phase:"start",x:session.x,y:session.y});
    requestAnimationFrame(() => dragScroll(session));
    return true;
  }

  function receiveDrag(raw) {
    if (raw.type === "pointerdown") {
      if (raw.button !== 0 || !raw.isPrimary) return;
      // WebKit on macOS does not focus buttons on a mouse press. Give enabled
      // tab stops actual focus before the editor loses it, so focus-within forms
      // can distinguish Cancel/Add from an outside click. Programmatic-only
      // controls retain their explicit non-pointer focus policy.
      const button=raw.target.closest?.("button");
      const owner=button && origin(button);
      if(!raw.defaultPrevented && owner && alive(owner)
          && !button.disabled && button.tabIndex>=0 && button.getAttribute("aria-disabled")!=="true") {
        // Cancel WebKit's subsequent default mouse focus clearing. Click still
        // activates on release; no activation is sent from this press handler.
        // A canceled press also suppresses native dragstart on draggable rows
        // containing buttons. Leave the drag gesture to the WebView there.
        if (!button.closest('[draggable="true" i]')) raw.preventDefault();
        button.focus({preventScroll:true});
      }
      suppressClick = false;
      if (drag) finishDrag("cancel",drag.x,drag.y);
      const entry = dragObserver(raw.target);
      if (!entry || dragBlocked(entry)) return;
      const element = main(entry), capture = element.getAttribute("data-gly-drag-capture");
      if (!capture || raw.target.closest?.("input,textarea,select,[contenteditable=true]")) return;
      const threshold = thresholdCapture(capture);
      if (!threshold && raw.target.closest?.("button,a")) return;
      drag = {e:entry,element,pointer:raw.pointerId,x:raw.clientX,y:raw.clientY,
              threshold,focus:capture==="press-focus",showPreview:capture==="threshold-preview",started:false};
      if (!threshold) startDrag(raw,drag);
      return;
    }
    if (!drag || raw.pointerId !== drag.pointer) return;
    const x = raw.clientX, y = raw.clientY;
    if (dragBlocked(drag.e) || raw.type === "pointercancel" || raw.type === "lostpointercapture") {
      finishDrag("cancel",x,y);
      return;
    }
    if (raw.type === "pointermove") {
      if (!drag.started && (Math.hypot(x-drag.x,y-drag.y) < 6 || !startDrag(raw,drag))) return;
      raw.preventDefault();
      drag.x=x; drag.y=y;
      event(drag.e,{kind:"drag",phase:"move",x,y});
    } else if (raw.type === "pointerup") {
      if (drag.started) {
        raw.preventDefault();
        suppressClick = true;
      }
      finishDrag("drop",x,y);
    }
  }
  window.addEventListener("blur",()=>{if(drag)finishDrag("cancel",drag.x,drag.y);});
  window.addEventListener("keydown",raw=>{if(raw.key==="Escape" && drag){if(drag.started)raw.preventDefault();finishDrag("cancel",drag.x,drag.y);}},true);
  // Wry supports nonmodal context-point layers. The DOM owns their top-layer
  // placement, while the widget owns visibility, item selection and commands.
  let openContext = null;
  function contextDismiss(reason) {
    if (openContext && !openContext.dismissed) {
      openContext.dismissed = true;
      event(openContext.entry,{kind:"dismiss",reason});
    }
  }

  function syncContextLayers() {
    const next = root.querySelector('[data-gly-layer="context-point"]:not([hidden])');
    if (openContext && (openContext.panel !== next || !next?.isConnected)) {
      const {panel, anchor} = openContext;
      const restore = panel.contains(document.activeElement) || document.activeElement === document.body;
      panel.hidePopover?.();
      openContext = null;
      if (!next && restore && anchor?.isConnected) anchor.focus({preventScroll:true});
    }
    if (!next) return;
    const anchor = document.getElementById(next.dataset.glyContextAnchor);
    if (anchor && !anchor.closest("[hidden],[inert],[data-gly-hidden]") && anchor.getClientRects().length) {
      const opening = !openContext;
      if (opening) {
        openContext = {panel:next,anchor,entry:origin(next),dismissed:false};
        next.showPopover();
      }
      const clamp = (point, space) => Math.max(8, Math.min(Number(point), space - 8));
      next.style.setProperty("--gly-layer-x", clamp(next.dataset.glyContextX, innerWidth-next.offsetWidth)+"px");
      next.style.setProperty("--gly-layer-y", clamp(next.dataset.glyContextY, innerHeight-next.offsetHeight)+"px");
      if (opening) (next.querySelector('[data-gly-menu-item]:not(:disabled):not([aria-disabled=true])') || next).focus({preventScroll:true});
    } else if (openContext) {
      contextDismiss("anchor_lost");
    } else {
      const entry = origin(next);
      if (entry?.interests.has("Dismiss")) event(entry,{kind:"dismiss",reason:"anchor_lost"});
    }
  }

  function receiveContext(raw, keyboard) {
    const target = raw.target.closest?.("[data-gly-context]");
    const entry = origin(target);
    if (!entry?.interests.has("ContextMenu") || dragBlocked(entry)) return false;
    if (target.dataset.glyContext === "native-editors" &&
        raw.target.closest?.("input,textarea,select,[contenteditable=true]")) return false;
    const rect = keyboard ? target.getBoundingClientRect() : null;
    raw.preventDefault();
    raw.stopImmediatePropagation();
    if (openContext && openContext.anchor !== target) contextDismiss("outside");
    event(entry,{kind:"context_menu",keyboard,x:rect ? rect.left : raw.clientX,y:rect ? rect.bottom : raw.clientY});
    return true;
  }

  window.addEventListener("resize",()=>{if(openContext)syncContextLayers();});
  document.addEventListener("pointerdown",raw=>{
    if (openContext && !openContext.panel.contains(raw.target)) contextDismiss("outside");
  },true);
  document.addEventListener("focusin",raw=>{
    if (openContext && !openContext.panel.contains(raw.target)) contextDismiss("outside");
  },true);
  window.addEventListener("keydown",raw=>{
    if (openContext && raw.key === "Escape" && !raw.isComposing) {
      raw.preventDefault(); raw.stopImmediatePropagation(); contextDismiss("escape");
    }
  },true);

  let movedMenuItem = null;
  function receiveMenuPointer(raw) {
    const item = raw.target.closest?.("[data-gly-menu-item]");
    const entry = origin(item);
    if (!entry || !openContext?.panel.contains(item) || dragBlocked(entry)) return;
    const phase = {pointerover:"enter",pointerout:"leave",pointermove:"move"}[raw.type];
    if (phase !== "move" && item.contains(raw.relatedTarget)) return;
    if (phase === "move" && movedMenuItem === item) return;
    movedMenuItem = phase === "move" ? item : null;
    if (entry.interests.has("Pointer") || observed(entry,"PointerWithin")) {
      event(entry,{kind:"pointer",phase});
    }
  }

  function receive(domEvent) {
    const e=origin(domEvent.target);
    if(!e || !alive(e)) return;
    const input=control(e), element=main(e);
    const disabled=!input || input.disabled || element.getAttribute("aria-disabled")==="true";
    switch(domEvent.type) {
      case "contextmenu": receiveContext(domEvent,false);break;
      case "click":
        if(suppressClick){suppressClick=false;domEvent.preventDefault();domEvent.stopPropagation();return;}
        if(disabled || input.readOnly && e.edit==="Check") {domEvent.preventDefault();return;}
        if(e.prevent || element.tagName==="A") domEvent.preventDefault();
        if(observed(e,"Activate")) event(e,{kind:"activate"});break;
      case "input":
        if(e.edit!=="Text" || disabled || input.readOnly) return;
        if(!e.composing && domEvent.isComposing) event(e,{kind:"composition",active:true});
        e.composing ||= !!domEvent.isComposing;
        if(e.interests.has("ValueChanged")) event(e,{kind:"value",value:readValue(e),revision:e.revision,composing:e.composing});break;
      case "change":
        if (e.edit==="Select" && !disabled && e.interests.has("SelectChanged")) {
          const keys = Array.from(input.selectedOptions)
            .filter(option => option.hasAttribute("data-gly-option")).map(option => option.dataset.glyOption);
          event(e,{kind:"select",keys,revision:e.revision});
        }
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
        const chord=domEvent.ctrlKey || domEvent.metaKey;
        if (!chord && ((key==="F10" && domEvent.shiftKey) || (key==="ContextMenu" && !domEvent.shiftKey)) &&
            receiveContext(domEvent,true)) return;
        if(key==="Enter" && editable && input.tagName!=="TEXTAREA" && !disabled && !input.readOnly && e.interests.has("Commit")) {
          domEvent.preventDefault();event(e,{kind:"commit",value:readValue(e),revision:e.revision});
        }
        if(!editable && e.edit!=="Select" && !domEvent.ctrlKey && !domEvent.metaKey && !domEvent.shiftKey && observed(e,"Navigate")) {
          const command={ArrowUp:"previous",ArrowDown:"next",Home:"first",End:"last",PageUp:"page_previous",PageDown:"page_next"}[key];
          if(command) {domEvent.preventDefault();event(e,{kind:"navigate",command});return;}
        }
        if (!editable && !chord && key.length===1 && /\S/.test(key) &&
            openContext?.panel.contains(domEvent.target) && observed(e,"Typeahead")) {
          domEvent.preventDefault();
          event(e,{kind:"typeahead",text:key,at_millis:Math.floor(performance.now())});
          return;
        }
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
    for(const name of ["contextmenu","click","input","change","compositionstart","compositionend","focusin","focusout","scroll","keydown"])
      root.addEventListener(name,receive,true);
    for(const name of ["pointerdown","pointermove","pointerup","pointercancel","lostpointercapture"])root.addEventListener(name,receiveDrag,true);
    for (const name of ["pointerover","pointerout","pointermove"])
      root.addEventListener(name,receiveMenuPointer,true);
    post({type:"ready"});
  }
  if(document.readyState==="loading") document.addEventListener("DOMContentLoaded",start,{once:true}); else start();
}
