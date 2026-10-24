/* Glory HTML stream v2. Classic external script: no eval or inline executable
 * code. Each invocation follows one completely parsed inert JSON data block. */
(() => {
  'use strict';
  if (!globalThis.GloryStream) {
    const controllers = new Map(), notes = new WeakMap();
    const MAX_BYTES = 8 * 1024 * 1024, MAX_NODES = 100000;
    const tags = new Set(['div', 'main', 'header', 'footer', 'nav', 'section', 'form', 'fieldset', 'legend', 'span', 'code', 'kbd', 'mark', 'blockquote', 'ol', 'ul', 'li', 'dl', 'dt', 'dd', 'p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'button', 'input', 'textarea', 'select', 'label', 'img', 'a', 'svg', 'path']);
    const fail = message => { throw new Error(`Glory stream: ${message}`); };
    const key = value => {
      if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value) || BigInt(value) > 18446744073709551615n) fail('invalid key');
      return value;
    };
    const note = element => {
      let value = notes.get(element);
      if (!value) { value = { dirty: new Set(), composing: false, deferred: new Map() }; notes.set(element, value); }
      return value;
    };
    // Installed by the head bootstrap before shell parsing: composition that
    // starts before the first data block is not lost while WASM is loading.
    const editEvent = event => {
      const element = event.target;
      if (!(element instanceof Element)) return;
      const root = element.closest('[data-gly-stream-pending],[data-gly-stream-state="ready"]');
      if (!root || !['INPUT', 'TEXTAREA', 'SELECT'].includes(element.tagName)) return;
      if (root.hasAttribute('data-gly-root') && !event.type.startsWith('composition')) return;
      const state = note(element);
      if (event.type === 'compositionstart') state.composing = true;
      else if (event.type === 'compositionend') { state.composing = false; state.dirty.add('value'); }
      else {
        state.dirty.add(element.tagName === 'SELECT' ? 'selectedKeys' : ['checkbox','radio'].includes(element.type) ? 'checked' : 'value');
        if (['checkbox','radio'].includes(element.type)) state.dirty.add('indeterminate');
        if (element.type === 'radio' && element.name) {
          // Native radios uncheck their peers without firing a change event on
          // those peers. Protect the whole live native group until adoption.
          for (const peer of document.getElementsByName(element.name)) {
            if (peer instanceof HTMLInputElement && peer.type === 'radio' && peer.form === element.form && peer.getRootNode() === element.getRootNode() && root.contains(peer)) {
              note(peer).dirty.add('checked'); note(peer).dirty.add('indeterminate');
            }
          }
        }
        if (event.isComposing) state.composing = true;
      }
      element[Symbol.for('glory.preload.composition')] = state.composing;
      controllers.get(root)?.settle(element);
    };
    for (const type of ['input', 'change', 'compositionstart', 'compositionend']) document.addEventListener(type, editEvent, true);
    // Server forms are application requests, including before WASM adoption.
    // Never let a pending streamed form navigate or reset dirty controls.
    for (const type of ['submit','reset']) document.addEventListener(type,event=>{
      if(event.target instanceof Element && event.target.matches('form[data-gly-form]')
        && event.target.closest('[data-gly-stream-pending],[data-gly-stream-state="ready"]')) event.preventDefault();
    },true);
    document.addEventListener('click', event => {
      const link = event.target instanceof Element && event.target.closest('a[data-gly-link-request]');
      if (link && link.closest('[data-gly-stream-pending],[data-gly-stream-state="ready"]')) event.preventDefault();
    }, true);
    function focusState() {
      const element = document.activeElement;
      let selection = null;
      try { if (typeof element?.selectionStart === 'number') selection = [element.selectionStart, element.selectionEnd, element.selectionDirection]; } catch (_) { /* nontext input */ }
      return { element, selection };
    }
    function restoreFocus(saved) {
      if (!saved?.element?.isConnected) return;
      if (document.activeElement !== saved.element) saved.element.focus({ preventScroll: true });
      if (saved.selection) saved.element.setSelectionRange(...saved.selection);
    }
    const selectedKeys = node => Array.from(node.options).filter(option => option.selected && option.hasAttribute('data-gly-option')).map(option => option.getAttribute('data-gly-option'));
    const same = (left, right) => Array.isArray(left) && Array.isArray(right) ? left.length === right.length && left.every((value, index) => value === right[index]) : left === right;
    const get = (node, name) => name === 'selectedKeys' ? selectedKeys(node) : node[name];
    const put = (node, name, value, defaults = true) => {
      if (name !== 'selectedKeys') { node[name] = value; return; }
      if (!(node instanceof HTMLSelectElement) || node.options.length > 1025) fail('select property target');
      const known = Array.from(node.options).filter(option=>option.hasAttribute('data-gly-option')).map(option=>option.getAttribute('data-gly-option'));
      if ((!node.multiple && value.length > 1) || value.some((key,index)=>!known.includes(key) || known.indexOf(key) <= (index ? known.indexOf(value[index-1]) : -1))) fail('select property keys/order');
      for (const option of node.options) { const selected = option.hasAttribute('data-gly-option') ? value.includes(option.getAttribute('data-gly-option')) : value.length === 0; if (defaults) { if (selected) option.setAttribute('selected',''); else option.removeAttribute('selected'); } option.selected = selected; }
    };
    const selectOptions = (node, config, keys) => {
      const option = (parent, value) => {
        const item = document.createElement('option'); item.setAttribute('data-gly-option', value.key); item.setAttribute('value', value.value);
        if (value.disabled) item.setAttribute('disabled','');
        if (keys.includes(value.key)) item.setAttribute('selected','');
        item.textContent = value.label; parent.append(item);
      };
      if (config.placeholder !== null) {
        const item = document.createElement('option'); for (const name of ['data-gly-placeholder','value','disabled','hidden']) item.setAttribute(name,'');
        if (!keys.length) item.setAttribute('selected',''); item.textContent = config.placeholder; node.append(item);
      }
      for (const value of config.entries) {
        if (!value.options) option(node, value);
        else {
          const group = document.createElement('optgroup'); group.setAttribute('label',value.label); if (value.disabled) group.setAttribute('disabled','');
          for (const value of value.options) option(group, value); node.append(group);
        }
      }
    };
    class Controller {
      constructor(root, initial) {
        if (!(root instanceof Element) || root.namespaceURI !== 'http://www.w3.org/1999/xhtml' || !root.isConnected || controllers.has(root) || root.hasAttribute('data-gly-root') || !root.hasAttribute('data-gly-stream-pending')) fail('root already claimed or unavailable');
        if (initial.version !== 2 || initial.html_version !== 6 || initial.kind !== 'start' || initial.sequence !== 0 || !Array.isArray(initial.nodes) || initial.nodes.length > MAX_NODES) fail('start envelope');
        if (typeof initial.data_id !== 'string' || !initial.data_id || initial.data_id === root.id) fail('data id');
        this.root = root; this.dataId = initial.data_id; this.entries = new Map(); this.layers = new Set(); this.composing = new Set(); this.sequence = 1; this.phase = 'active'; this.peak = 0;
        this.first = root.firstChild; this.last = root.lastChild;
        if (this.first?.nodeValue !== 'gly:stream:2' || this.last?.nodeValue !== 'gly:/stream:2') fail('pending boundaries');
        const parts = new Map(), comments = new Map();
        for (const element of root.querySelectorAll('[data-gly-key]')) {
          const id = key(element.getAttribute('data-gly-key')), part = element.getAttribute('data-gly-part');
          const idPart = `${id}/${part}`;
          if (parts.has(idPart) || element.hasAttribute('data-gly-node')) fail('duplicate/claimed element');
          parts.set(idPart, element);
        }
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);
        while (walker.nextNode()) {
          const text = walker.currentNode.nodeValue;
          if (/^gly:[set]:[0-9]+$/.test(text)) { if (comments.has(text)) fail('duplicate marker'); comments.set(text, walker.currentNode); }
        }
        for (const model of initial.nodes) {
          const id = key(model.key); if (this.entries.has(id)) fail('duplicate initial node');
          if (typeof model.placed !== 'boolean' || typeof model.suspended !== 'boolean' || !Array.isArray(model.children) || new Set(model.children).size !== model.children.length) fail('initial structure');
          this.validateShape(model.shape);
          const entry = { key: id, shape: model.shape, parent: model.parent === null ? null : key(model.parent), children: model.children.map(key), placed: model.placed, suspended: model.suspended, parts: Object.create(null), expected: Object.create(null), fragment: document.createDocumentFragment() };
          entry.first = parts.get(`${id}/main`) || comments.get(`gly:s:${id}`);
          entry.last = parts.get(`${id}/main`) || comments.get(`gly:e:${id}`);
          entry.existing = !!entry.first;
          if (entry.existing) {
            if (!entry.last || entry.first.parentNode !== entry.last.parentNode) fail('initial range');
            for (const piece of entry.shape.pieces) {
              if ('text' in piece) {
                entry.marker = comments.get(`gly:t:${id}`); if (!entry.marker) fail('text marker');
                let text = entry.marker.nextSibling;
                if (text?.nodeType !== Node.TEXT_NODE) { text = document.createTextNode(''); entry.marker.after(text); }
                if (text.nodeValue !== piece.text) fail('initial text'); entry.parts[piece.part] = text;
              } else {
                const element = parts.get(`${id}/${piece.part}`);
                if (!element || element.localName !== piece.tag || element.namespaceURI !== (['svg','path'].includes(piece.tag) ? 'http://www.w3.org/2000/svg' : 'http://www.w3.org/1999/xhtml')) fail('initial tag/part');
                const expected = { ...piece.attributes, 'data-gly-key': id, 'data-gly-part': piece.part };
                if (element.attributes.length !== Object.keys(expected).length || Object.entries(expected).some(([name, value]) => element.getAttribute(name) !== value)) fail('initial attributes');
                entry.parts[piece.part] = element;
                if (piece.tag === 'select') {
                  const probe = document.createElement('select'); selectOptions(probe, entry.shape.options, piece.properties.selectedKeys);
                  if (probe.childNodes.length !== element.childNodes.length || Array.from(probe.childNodes).some((node,index)=>!node.isEqualNode(element.childNodes[index]))) fail('initial select options');
                }
                this.baseline(entry, piece, element);
              }
            }
          } else this.createParts(entry);
          this.entries.set(id, entry);
          if (entry.parts.main?.hasAttribute('data-gly-layer')) this.layers.add(id);
        }
        // Build only detached missing trees. Never replay Place/value setters
        // over the already parsed visible shell.
        const arranged = new Set();
        const arrange = entry => {
          if (arranged.has(entry.key)) return; arranged.add(entry.key);
          for (const id of entry.children) {
            const child = this.entry(id); if (child.parent !== entry.key) fail('initial parent');
            if (child.existing && !entry.existing) fail('visible child of hidden parent');
            if (!child.existing && !entry.existing && child.placed && !child.suspended) this.move(child, ...this.container(entry));
            arrange(child);
          }
        };
        for (const entry of this.entries.values()) arrange(entry);
        for (const entry of this.entries.values()) {
          let active = true, depth = 0;
          for (let ancestor = entry; ancestor; ancestor = ancestor.parent === null ? null : this.entry(ancestor.parent)) { if (++depth > 512) fail('tree depth/cycle'); active &&= ancestor.placed && !ancestor.suspended; }
          if (active !== entry.existing) fail('initial visibility');
          for (const node of Object.values(entry.parts)) if (node instanceof Element && notes.get(node)?.composing) this.composing.add(node);
          if (!entry.existing) continue;
          const [parent] = this.container(entry.parent === null ? null : this.entry(entry.parent));
          if (entry.first.parentNode !== parent || !entry.placed || entry.suspended) fail('visible placement');
        }
        this.syncInert();
        this.peak = this.entries.size; controllers.set(root, this);
        this.observer = new MutationObserver(() => { if (!root.isConnected) this.cancel(); });
        this.observer.observe(document, { childList: true, subtree: true });
        root.dataset.glyStreamState = 'active';
      }
      entry(id) { const entry = this.entries.get(key(id)); if (!entry) fail(`missing node ${id}`); return entry; }
      syncInert() {
        // Only current layer declarations and their semantic ancestors. Group
        // ranges are transparent; there is no visited-node or business history.
        for (const id of this.layers) {
          const layer=this.entry(id); let blocked=false,depth=0;
          for(let entry=layer;entry;entry=entry.parent===null?null:this.entry(entry.parent)) {
            if(++depth>512) fail('inert ancestry depth');
            if(entry.parts.main?.hasAttribute('data-gly-inert')) {blocked=true;break;}
          }
          layer.parts.main.toggleAttribute('inert',blocked);
        }
      }
      watchParserEnd() {
        if (document.readyState !== 'loading') return;
        this.eof = () => {
          if (this.phase === 'active' && !this.pendingEnd) {
            this.phase = 'failed'; this.root.dataset.glyStreamState = 'failed'; this.release();
            this.root.dispatchEvent(new CustomEvent('glory:stream-error', { detail: 'Glory stream: response ended before End' }));
          }
        };
        document.addEventListener('DOMContentLoaded', this.eof, { once: true });
      }
      validateShape(shape) {
        if (!shape || !Array.isArray(shape.pieces) || shape.pieces.length > 3) fail('shape');
        if ((shape.pieces[0]?.tag === 'select') !== (shape.options !== null)) fail('select configuration target');
        if (shape.options !== null) {
          const config = shape.options;
          if (!config || !Array.isArray(config.entries) || config.entries.length > 1056 || !(config.placeholder === null || typeof config.placeholder === 'string' && config.placeholder.length <= 256)) fail('select options');
          let count = 0, groups = 0, bytes = new TextEncoder().encode(config.placeholder || '').length; const keys = new Set();
          const text = (value, limit) => typeof value === 'string' && new TextEncoder().encode(value).length <= limit && !/[\0\r]/.test(value);
          for (const entry of config.entries) {
            const values = entry.options || [entry];
            if (!Array.isArray(values) || values.length > 1024) fail('select option count');
            if (entry.options && (++groups > 32 || !entry.label || !text(entry.label,256) || typeof entry.disabled !== 'boolean')) fail('select group');
            if (entry.options) bytes += new TextEncoder().encode(entry.label).length;
            for (const value of values) {
              if (++count > 1024 || typeof value.key !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(value.key) || keys.has(value.key) || !text(value.label,256) || !text(value.value,4096) || typeof value.disabled !== 'boolean') fail('select option');
              keys.add(value.key); bytes += new TextEncoder().encode(value.key + value.label + value.value).length;
            }
          }
          if (bytes > 131072 || shape.pieces[0]?.tag !== 'select') fail('select bytes/target');
          const selection = shape.pieces[0].properties.selectedKeys;
          if (!Array.isArray(selection) || (!('multiple' in shape.pieces[0].attributes) && selection.length > 1) || selection.some(key=>!keys.has(key)) || selection.some((key,index)=>[...keys].indexOf(key) <= (index ? [...keys].indexOf(selection[index-1]) : -1))) fail('select keys/order');
        }
        const seen = new Set();
        for (const piece of shape.pieces) {
          if (!['main', 'control', 'content'].includes(piece.part) || seen.has(piece.part)) fail('part'); seen.add(piece.part);
          if ('text' in piece) { if (typeof piece.text !== 'string') fail('text'); }
          else {
            if (!tags.has(piece.tag) || !piece.attributes || !piece.properties) fail('element');
            for (const [name, value] of Object.entries(piece.attributes)) this.attributeName(name, value);
            for (const [name, value] of Object.entries(piece.properties)) this.propertyName(name, value);
          }
          if (piece.parent !== null && (piece.parent === piece.part || !seen.has(piece.parent))) fail('part parent');
        }
        if (shape.children !== null && !seen.has(shape.children)) fail('child slot');
      }
      attributeName(name, value) { if (typeof value !== 'string' && value !== null || !(['viewBox','preserveAspectRatio'].includes(name) || /^[a-z][a-z0-9-]*$/.test(name)) || name.startsWith('on') || ['srcdoc', 'data-gly-key', 'data-gly-part', 'data-gly-node', 'data-gly-live'].includes(name)) fail('attribute'); }
      propertyName(name, value) { if (name === 'selectedKeys') { if (!Array.isArray(value) || value.length > 1024 || new Set(value).size !== value.length || value.some(key => typeof key !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(key))) fail('selected keys'); return; } if (!['value', 'checked', 'disabled', 'indeterminate'].includes(name) || (name === 'value' ? typeof value !== 'string' : typeof value !== 'boolean')) fail('property'); }
      baseline(entry, piece, element) {
        for (const [name, value] of Object.entries(piece.properties)) {
          let expected = value;
          if (name === 'value') { const probe = element.cloneNode(false); probe.value = value; expected = probe.value; }
          entry.expected[`${piece.part}/${name}`] = expected;
          if (name === 'indeterminate' && !note(element).dirty.has(name) && !note(element).dirty.has('checked')) element[name] = expected;
          if (['value', 'checked', 'indeterminate', 'selectedKeys'].includes(name) && !same(get(element,name), expected)) note(element).dirty.add(name);
        }
      }
      createParts(entry) {
        for (const piece of entry.shape.pieces) {
          let node;
          const parent = piece.parent === null ? entry.fragment : entry.parts[piece.parent];
          if ('text' in piece) {
            entry.marker = document.createComment(`gly:t:${entry.key}`); parent.append(entry.marker); node = document.createTextNode(piece.text);
          } else {
            node = ['svg','path'].includes(piece.tag) ? document.createElementNS('http://www.w3.org/2000/svg',piece.tag) : document.createElement(piece.tag);
            for (const [name, value] of Object.entries(piece.attributes)) node.setAttribute(name, value);
            node.setAttribute('data-gly-key', entry.key); node.setAttribute('data-gly-part', piece.part);
            if (piece.tag === 'select') selectOptions(node, entry.shape.options, piece.properties.selectedKeys);
            for (const [name, value] of Object.entries(piece.properties)) put(node, name, value);
            this.baseline(entry, piece, node);
          }
          parent.append(node); entry.parts[piece.part] = node;
        }
        if (!entry.shape.pieces.length) entry.fragment.append(document.createComment(`gly:s:${entry.key}`), document.createComment(`gly:e:${entry.key}`));
        entry.first = entry.fragment.firstChild; entry.last = entry.fragment.lastChild;
      }
      container(entry) {
        if (entry === null) return [this.root, this.last];
        if (entry.shape.children) return [entry.parts[entry.shape.children], null];
        if (!entry.shape.pieces.length) return [entry.last.parentNode, entry.last];
        fail('no child slot');
      }
      range(entry) {
        const nodes = []; let node = entry.first;
        for (;;) { if (!node) fail('broken range'); nodes.push(node); if (node === entry.last) return nodes; node = node.nextSibling; }
      }
      move(entry, parent, before) {
        const nodes = this.range(entry), saved = focusState();
        for (const node of nodes) {
          if (parent.moveBefore && node.isConnected && parent.isConnected) {
            try { parent.moveBefore(node, before); } catch (_) { parent.insertBefore(node, before); }
          } else parent.insertBefore(node, before);
        }
        if (saved.element && nodes.some(node => node === saved.element || node.contains(saved.element))) restoreFocus(saved);
      }
      unlink(entry) { if (entry.parent !== null) { const parent = this.entry(entry.parent); parent.children = parent.children.filter(id => id !== entry.key); } }
      patch(entry, edits) {
        for (const edit of edits) {
          if (edit.edit === 'revision') continue;
          const node = entry.parts[edit.part]; if (!node) fail('patch part');
          if (edit.edit === 'text') { if (typeof edit.value !== 'string' || node.nodeType !== Node.TEXT_NODE) fail('text edit'); if (node.nodeValue !== edit.value) node.nodeValue = edit.value; }
          else if (edit.edit === 'attribute') {
            this.attributeName(edit.name, edit.value);
            const state = note(node), name = edit.name;
            if (state.composing && ['value', 'checked'].includes(name)) { state.deferred.set(name, edit.value); continue; }
            const previous = node[name], protectedValue = state.dirty.has(name) || (['value', 'checked'].includes(name) && previous !== entry.expected[`${edit.part}/${name}`]);
            if (protectedValue) state.dirty.add(name);
            if (edit.value === null) node.removeAttribute(name); else node.setAttribute(name, edit.value);
            if (protectedValue && ['value', 'checked'].includes(name) && node[name] !== previous) node[name] = previous;
          } else if (edit.edit === 'property') {
            this.propertyName(edit.name, edit.value);
            const slot = `${edit.part}/${edit.name}`, state = note(node);
            const dirty = ['value', 'checked', 'indeterminate', 'selectedKeys'].includes(edit.name) && (state.dirty.has(edit.name) || state.composing || !same(get(node,edit.name), entry.expected[slot]));
            let expected = edit.value;
            if (edit.name === 'value') { const probe = node.cloneNode(false); probe.value = edit.value; expected = probe.value; }
            entry.expected[slot] = expected;
            if (edit.name === 'selectedKeys') { const actual = get(node,edit.name); put(node,edit.name,expected); if (dirty) put(node,edit.name,actual,false); }
            else if (!dirty && !same(get(node,edit.name), expected)) put(node,edit.name,expected);
          } else fail('edit kind');
        }
        if(entry.parts.main?.hasAttribute('data-gly-layer')) this.layers.add(entry.key); else this.layers.delete(entry.key);
      }
      apply(frame) {
        try { return this.applyFrame(frame); }
        catch (error) { this.phase = 'failed'; this.root.dataset.glyStreamState = 'failed'; this.release(); throw error; }
      }
      applyFrame(frame) {
        if (this.phase !== 'active' || this.pendingEnd || frame.version !== 2 || frame.sequence !== this.sequence || !Number.isSafeInteger(frame.sequence)) fail('frame order/version');
        if (new TextEncoder().encode(JSON.stringify(frame)).length > MAX_BYTES) fail('frame bytes');
        if (frame.kind === 'end') { this.validateEnd(frame); this.sequence++; this.pendingEnd = frame; this.settle(); return; }
        if (frame.kind !== 'patch' || !Array.isArray(frame.ops)) fail('patch envelope');
        for (const op of frame.ops) {
          if (op.op === 'create') {
            const id = key(op.key); if (this.entries.has(id) || this.entries.size >= MAX_NODES) fail('duplicate/too many nodes'); this.validateShape(op.shape);
            const entry = { key: id, shape: op.shape, parts: Object.create(null), expected: Object.create(null), parent: null, children: [], placed: false, suspended: false, fragment: document.createDocumentFragment() };
            this.createParts(entry); this.entries.set(id, entry); if(entry.parts.main?.hasAttribute('data-gly-layer')) this.layers.add(id); this.peak = Math.max(this.peak, this.entries.size);
          } else {
            const entry = this.entry(op.key);
            if (op.op === 'patch') this.patch(entry, op.edits);
            else if (op.op === 'place') {
              const parent = op.parent === null ? null : this.entry(op.parent), before = op.before === null ? null : this.entry(op.before);
              if (before && (before === entry || before.parent !== op.parent)) fail('before parent');
              for (let ancestor = parent; ancestor; ancestor = ancestor.parent === null ? null : this.entry(ancestor.parent)) if (ancestor === entry) fail('placement cycle');
              this.unlink(entry); entry.parent = op.parent; entry.placed = true; entry.suspended = false;
              if (parent) { const at = before ? parent.children.indexOf(before.key) : parent.children.length; parent.children.splice(at, 0, entry.key); }
              const [container, end] = this.container(parent); this.move(entry, container, before ? before.first : end);
              if (entry.savedFocus && document.activeElement === document.body) restoreFocus(entry.savedFocus); entry.savedFocus = null;
            } else if (op.op === 'suspend') {
              const nodes = this.range(entry), saved = focusState();
              entry.savedFocus = nodes.some(node => node === saved.element || node.contains(saved.element)) ? saved : null;
              for (const node of nodes) entry.fragment.append(node); entry.suspended = true;
              for (const node of [...this.composing]) if (!node.isConnected) { note(node).composing = false; this.composing.delete(node); }
            } else if (op.op === 'dispose') {
              const actual = new Set(); const collect = node => { actual.add(node.key); for (const id of node.children) collect(this.entry(id)); }; collect(entry);
              if (!Array.isArray(op.subtree) || op.subtree.length !== actual.size || op.subtree.some(id => !actual.has(key(id)))) fail('dispose subtree');
              this.unlink(entry); for (const node of this.range(entry)) node.remove();
              for (const id of actual) { for (const node of Object.values(this.entry(id).parts)) this.composing.delete(node); this.entries.delete(id); this.layers.delete(id); }
            } else fail('operation');
          }
        }
        this.syncInert(); this.sequence++;
        this.root.dispatchEvent(new CustomEvent('glory:stream-patch', { detail: { sequence: frame.sequence, nodes: this.entries.size } }));
      }
      validateEnd(frame) {
        if (frame.html_version !== 6 || !Array.isArray(frame.rekey) || frame.rekey.length !== this.entries.size) fail('end keys/version');
        const old = new Set(), next = new Set();
        for (const pair of frame.rekey) { if (!Array.isArray(pair) || pair.length !== 2 || old.has(key(pair[0])) || next.has(key(pair[1])) || !this.entries.has(pair[0])) fail('rekey'); old.add(pair[0]); next.add(pair[1]); }
        if (document.getElementById(this.dataId) || !frame.resources || frame.resources.version !== 1 || !Array.isArray(frame.resources.records) || frame.resources.records.length > 4096) fail('resource envelope/data id');
        const names = new Set();
        for (const record of frame.resources.records) { if (typeof record.name !== 'string' || !record.name || names.has(record.name) || typeof record.schema !== 'string' || !record.schema || !['ready', 'failed'].includes(record.result?.state)) fail('resource record'); names.add(record.name); }
      }
      settle(element) {
        try { return this.settleFrame(element); }
        catch (error) { this.phase = 'failed'; this.root.dataset.glyStreamState = 'failed'; this.release(); throw error; }
      }
      settleFrame(element) {
        const nodes = element ? [element] : [...this.entries.values()].flatMap(entry => Object.values(entry.parts));
        for (const node of nodes) {
          if (!(node instanceof Element)) continue; const state = notes.get(node); if (!state) continue;
          if (state.composing) { this.composing.add(node); continue; } this.composing.delete(node);
          for (const [name, value] of state.deferred) { const current = node[name]; if (value === null) node.removeAttribute(name); else node.setAttribute(name, value); if (node[name] !== current) node[name] = current; }
          state.deferred.clear();
        }
        if (!this.pendingEnd || this.composing.size) return;
        const frame = this.pendingEnd; this.validateEnd(frame);
        for (const [old, next] of frame.rekey) {
          const entry = this.entry(old);
          for (const node of Object.values(entry.parts)) if (node instanceof Element) node.setAttribute('data-gly-key', next);
          if (entry.marker) entry.marker.nodeValue = `gly:t:${next}`;
          if (!entry.shape.pieces.length) { entry.first.nodeValue = `gly:s:${next}`; entry.last.nodeValue = `gly:e:${next}`; }
        }
        const data = document.createElement('script'); data.id = this.dataId; data.type = 'application/json'; data.setAttribute('data-gly-resources', '1'); data.textContent = JSON.stringify(frame.resources);
        this.root.after(data); this.first.nodeValue = 'gly:v:6'; this.last.nodeValue = 'gly:/v:6';
        this.phase = 'ready'; this.root.removeAttribute('data-gly-stream-pending'); this.root.dataset.glyStreamState = 'ready';
        this.release(); this.root.dispatchEvent(new CustomEvent('glory:stream-ready', { detail: { dataId: this.dataId, peak: this.peak } }));
      }
      release() { controllers.delete(this.root); this.observer?.disconnect(); if (this.eof) document.removeEventListener('DOMContentLoaded', this.eof); this.eof = null; this.entries.clear(); this.layers.clear(); this.composing.clear(); this.pendingEnd = null; }
      cancel() { if (!['active'].includes(this.phase)) return; this.phase = 'cancelled'; this.root.dataset.glyStreamState = 'cancelled'; this.release(); }
    }
    globalThis.GloryStream = Object.freeze({
      start(root, data) { return new Controller(root, data); },
      active() { return controllers.size; },
      cancel(root) { controllers.get(root)?.cancel(); },
      receive(block) {
        if (!(block instanceof HTMLScriptElement) || block.type !== 'application/json' || !block.hasAttribute('data-gly-stream')) fail('data block');
        const root = document.getElementById(block.getAttribute('data-gly-stream'));
        if (!root) fail('root missing');
        if (root.dataset.glyStreamState === 'cancelled' || root.dataset.glyStreamState === 'failed') return;
        try {
          if (new TextEncoder().encode(block.textContent).length > MAX_BYTES) fail('data block bytes');
          const data = JSON.parse(block.textContent);
          if (data.kind === 'start') new Controller(root, data).watchParserEnd(); else { const controller = controllers.get(root); if (!controller) fail('no stream'); controller.apply(data); }
        } catch (error) { controllers.get(root)?.cancel(); root.dataset.glyStreamState = 'failed'; root.dispatchEvent(new CustomEvent('glory:stream-error', { detail: String(error) })); throw error; }
      },
    });
  }
  const script = document.currentScript, block = script?.previousElementSibling;
  if (block?.matches('script[data-gly-stream]')) {
    try { globalThis.GloryStream.receive(block); } finally { block.remove(); script.remove(); }
  }
})();
