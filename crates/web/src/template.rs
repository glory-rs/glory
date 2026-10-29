//! Experimental instance/slot display runtime. No semantic Memory/Mapper/Entry
//! per HTML element. The current profile is documented in html::template.
pub use crate::view::{Error, Result};
use glory_core::{BindingId, Host, HostError, InstanceId, LocalFuture, Runtime, Scope, Vary};
use glory_html::{
    template::Kind,
    view::{Compiled, Template},
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
    sync::Arc,
    time::Duration,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};

#[wasm_bindgen(module = "/src/template_host.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = createHost)]
    fn create_host(
        root: &web_sys::Element,
        ingress: &js_sys::Function,
    ) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = defineTemplate)]
    fn define_template(host: &JsValue, wire: &str) -> std::result::Result<u32, JsValue>;
    #[wasm_bindgen(catch, js_name = submit)]
    fn submit(
        host: &JsValue,
        records: &[u32],
        strings: &str,
    ) -> std::result::Result<js_sys::Array, JsValue>;
    #[wasm_bindgen(catch, js_name = destroyHost)]
    fn destroy_host(host: &JsValue) -> std::result::Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = childContainer)]
    fn child_container(
        host: &JsValue,
        id: u32,
        slot: u16,
    ) -> std::result::Result<web_sys::Element, JsValue>;
}
fn backend(value: JsValue) -> Error {
    Error(format!("template host: {value:?}"))
}
fn error(value: impl std::fmt::Display) -> Error {
    Error(value.to_string())
}
type Handler = Rc<RefCell<Box<dyn FnMut()>>>;
// Sparse-only apps do not retain a semantic renderer's methods or drop graph.
trait Island {
    fn flush(&self) -> Result<()>;
}
impl Island for crate::WebApp {
    fn flush(&self) -> Result<()> {
        if self.app().scope().is_alive() {
            crate::WebApp::flush(self).map_err(error)?;
        }
        Ok(())
    }
}
type PendingWidget =
    Box<dyn FnOnce(web_sys::Element, Rc<Runtime>, InstanceId) -> Result<Rc<dyn Island>>>;
struct Record {
    template: Arc<Compiled>,
    owner: InstanceId,
    owner_parent: Option<InstanceId>,
    parent: Option<(u32, u16)>,
    placed: bool,
    handlers: HashMap<u16, Handler>,
    bindings: Vec<BindingId>,
    node_count: usize,
    children: Vec<Scope>,
}
struct Declaration {
    data: Arc<Compiled>,
    host_id: Cell<Option<u32>>,
}
enum Op {
    Create(u32, u32, usize, usize),
    Write(u32, u16, usize, usize, u32),
    Flag(u32, u16, bool),
    Place(u32, Option<(u32, u16)>, Option<u32>),
    Retire(u32, usize, usize),
}
struct Bridge {
    state: RefCell<Weak<State>>,
    clock: web_sys::Performance,
}
impl Host for Bridge {
    fn spawn(&self, future: LocalFuture) -> std::result::Result<Box<dyn FnOnce()>, HostError> {
        let (abort, registration) = futures_util::future::AbortHandle::new_pair();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = futures_util::future::Abortable::new(future, registration).await;
        });
        Ok(Box::new(move || abort.abort()))
    }
    fn wake(&self) {
        if let Some(state) = self.state.borrow().upgrade() {
            state.schedule();
        }
    }
    fn now(&self) -> Duration {
        Duration::from_secs_f64(self.clock.now() / 1000.)
    }
}
struct State {
    runtime: Rc<Runtime>,
    bridge: Rc<Bridge>,
    scope: Rc<Scope>,
    host: JsValue,
    ingress: RefCell<Option<Closure<dyn FnMut(u32, u32)>>>,
    pending: Cell<bool>,
    closed: Cell<bool>,
    fault: RefCell<Option<Error>>,
    events: RefCell<VecDeque<(u32, u16)>>,
    records: RefCell<HashMap<u32, Record>>,
    widgets: RefCell<HashMap<u32, Rc<dyn Island>>>,
    pending_widgets: RefCell<HashMap<u32, PendingWidget>>,
    owners: RefCell<HashMap<InstanceId, u32>>,
    templates: RefCell<HashMap<usize, Rc<Declaration>>>,
    next: Cell<u32>,
    commands: RefCell<Vec<Op>>,
    texts: RefCell<String>,
    applied: Cell<u64>,
    instances: Cell<usize>,
    targets: Cell<usize>,
    flushing: Cell<bool>,
}
/// One display document. Callbacks should capture WeakApp/Instance, not App.
#[derive(Clone)]
pub struct App(Rc<State>);
#[derive(Clone)]
pub struct WeakApp(Weak<State>);
#[derive(Clone)]
pub struct Instance {
    state: Weak<State>,
    id: u32,
}
#[derive(Clone)]
pub struct Children {
    instance: Instance,
    slot: u16,
}
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub instances: usize,
    pub targets: usize,
    pub records: usize,
    pub templates: usize,
    pub applied: u64,
}

impl App {
    pub fn at_id(id: &str) -> Result<Self> {
        let root = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .ok_or_else(|| error(format!("missing display root: {id}")))?;
        Self::new(root)
    }
    pub fn launch(id: &str, widget: impl glory_ui::Widget) -> Result<Self> {
        let app = Self::at_id(id)?;
        app.render(widget)?;
        Ok(app)
    }
    pub fn new(root: web_sys::Element) -> Result<Self> {
        Self::create(root)
    }
    fn create(root: web_sys::Element) -> Result<Self> {
        let window = web_sys::window().ok_or_else(|| error("missing Window"))?;
        let clock = window
            .performance()
            .ok_or_else(|| error("missing Performance clock"))?;
        let bridge = Rc::new(Bridge {
            state: RefCell::new(Weak::new()),
            clock,
        });
        let runtime = Rc::new(Runtime::with_host(bridge.clone()));
        let scope = Rc::new(runtime.scope());
        let ingress_state: Rc<RefCell<Weak<State>>> = Rc::new(RefCell::new(Weak::new()));
        let slot = ingress_state.clone();
        let ingress = Closure::new(move |id, target: u32| {
            if let Some(state) = slot.borrow().upgrade() {
                if state.closed.get() || state.fault.borrow().is_some() {
                    return;
                }
                if state.events.borrow().len() >= 4096 {
                    state.fail(error("template event budget"));
                    return;
                }
                state.events.borrow_mut().push_back((id, target as u16));
                state.schedule();
            }
        });
        let host = create_host(&root, ingress.as_ref().unchecked_ref()).map_err(backend)?;
        let state = Rc::new(State {
            runtime,
            bridge: bridge.clone(),
            scope,
            host,
            ingress: RefCell::new(Some(ingress)),
            pending: Cell::new(false),
            closed: Cell::new(false),
            fault: RefCell::new(None),
            events: RefCell::new(VecDeque::new()),
            records: RefCell::new(HashMap::new()),
            widgets: RefCell::new(HashMap::new()),
            pending_widgets: RefCell::new(HashMap::new()),
            owners: RefCell::new(HashMap::new()),
            templates: RefCell::new(HashMap::new()),
            next: Cell::new(1),
            commands: RefCell::new(Vec::new()),
            texts: RefCell::new(String::new()),
            applied: Cell::new(0),
            instances: Cell::new(0),
            targets: Cell::new(0),
            flushing: Cell::new(false),
        });
        *bridge.state.borrow_mut() = Rc::downgrade(&state);
        *ingress_state.borrow_mut() = Rc::downgrade(&state);
        Ok(Self(state))
    }
    pub fn scope(&self) -> Rc<Scope> {
        self.0.scope.clone()
    }
    pub fn downgrade(&self) -> WeakApp {
        WeakApp(Rc::downgrade(&self.0))
    }
    pub fn runtime(&self) -> &Runtime {
        &self.0.runtime
    }
    pub fn flush(&self) -> Result<()> {
        self.0.flush()
    }
    pub fn stats(&self) -> Stats {
        Stats {
            instances: self.0.instances.get(),
            targets: self.0.targets.get(),
            records: self.0.records.borrow().len(),
            templates: self.0.templates.borrow().len(),
            applied: self.0.applied.get(),
        }
    }
    pub fn mount(&self, owner: &Scope, template: &'static Template) -> Result<Instance> {
        self.0.mount(owner, template)
    }
    pub(crate) fn prepare(&self, template: &'static Template) -> Result<Arc<Compiled>> {
        Ok(self.0.declaration(template)?.data.clone())
    }
    pub(crate) fn ensure_live(&self) -> Result<()> {
        self.0.check()
    }
    pub fn render(&self, view: impl glory_ui::Widget) -> Result<crate::view::Mounted> {
        let instance = crate::view::mount(view, self, &self.0.scope)?;
        instance.place(None, None)?;
        self.flush()?;
        Ok(instance)
    }
    pub(crate) fn mount_widget(
        &self,
        owner: &Scope,
        widget: impl glory_ui::Widget,
    ) -> Result<Instance> {
        use glory_html::view::{Outlet, Template, children, div};
        static ISLAND: Template = Template::new(&[div(&[children(Outlet::new("widget"))], &[])]);
        let instance = self.mount(owner, &ISLAND)?;
        self.0.pending_widgets.borrow_mut().insert(
            instance.id,
            Box::new(move |root, runtime, owner| {
                crate::browser::launch_in_runtime(root, runtime, owner, widget)
                    .map(|app| Rc::new(app) as Rc<dyn Island>)
                    .map_err(error)
            }),
        );
        Ok(instance)
    }
    pub fn close(&self) -> Result<()> {
        if self.0.flushing.get() {
            return Err(error("template flush already in progress"));
        }
        self.0.close()
    }
}
impl WeakApp {
    pub fn upgrade(&self) -> Result<App> {
        self.0
            .upgrade()
            .map(App)
            .ok_or_else(|| error("template app gone"))
    }
}
impl State {
    fn check(&self) -> Result<()> {
        if self.closed.get() {
            return Err(error("template app closed"));
        }
        if let Some(e) = self.fault.borrow().as_ref() {
            return Err(e.clone());
        }
        Ok(())
    }
    fn fail(&self, e: Error) {
        self.fault.borrow_mut().get_or_insert(e);
    }
    fn schedule(self: &Rc<Self>) {
        if self.closed.get() || self.fault.borrow().is_some() || self.pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        // Coalesce one JavaScript turn without imposing an extra animation frame.
        // The task never retains the App; close/fault revokes its work.
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(state) = weak.upgrade() {
                state.pending.set(false);
                let live = !state.closed.get() && state.fault.borrow().is_none();
                if live && let Err(e) = state.flush() {
                    state.fail(e)
                }
            }
        });
    }
    fn queue(&self, op: Op) -> Result<()> {
        self.check()?;
        let mut commands = self.commands.borrow_mut();
        if commands.len() >= 1_048_576 {
            return Err(error("template command budget"));
        }
        commands.push(op);
        if !self.flushing.get() {
            self.bridge.wake();
        }
        Ok(())
    }
    fn queue_text(&self, id: u32, slot: u16, value: &str) -> Result<()> {
        self.check()?;
        if value.len() > 64 * 1024 {
            return Err(error("template slot payload budget"));
        }
        let mut commands = self.commands.try_borrow_mut().map_err(error)?;
        if commands.len() >= 1_048_576 {
            return Err(error("template command budget"));
        }
        let mut texts = self.texts.try_borrow_mut().map_err(error)?;
        if texts.len() + value.len() > 64 * 1024 * 1024 {
            return Err(error("template pending text budget"));
        }
        let start = texts.len();
        texts.push_str(value);
        commands.push(Op::Write(
            id,
            slot,
            start,
            texts.len(),
            value.encode_utf16().count() as u32,
        ));
        if !self.flushing.get() {
            self.bridge.wake();
        }
        Ok(())
    }
    fn declaration(&self, template: &'static Template) -> Result<Rc<Declaration>> {
        self.check()?;
        let key = template as *const Template as usize;
        if let Some(known) = self.templates.borrow().get(&key) {
            return Ok(known.clone());
        }
        if self.templates.borrow().len() >= 64 {
            return Err(error("template declaration budget"));
        }
        let declaration = Rc::new(Declaration {
            data: template.cached().map_err(error)?,
            host_id: Cell::new(None),
        });
        self.templates.borrow_mut().insert(key, declaration.clone());
        Ok(declaration)
    }
    fn mount(self: &Rc<Self>, owner: &Scope, template: &'static Template) -> Result<Instance> {
        self.check()?;
        if owner.runtime() != self.runtime.id() || !owner.is_alive() {
            return Err(error("foreign/dead template owner"));
        }
        if self.owners.borrow().contains_key(&owner.id()) {
            return Err(error("one template instance per Scope; use a child Scope"));
        }
        let declaration = self.declaration(template)?;
        let template = declaration.data.clone();
        let node_count = template.node_count;
        let template_id = if let Some(id) = declaration.host_id.get() {
            id
        } else {
            self.flush_host()?; // cold native work never overtakes queued writes
            let id = define_template(&self.host, &template.wire)
                .map_err(backend)
                .inspect_err(|e| self.fail(e.clone()))?;
            declaration.host_id.set(Some(id));
            id
        };
        let id = self.next.get();
        let next = id
            .checked_add(1)
            .ok_or_else(|| error("template instance space exhausted"))?;
        self.queue(Op::Create(
            id,
            template_id,
            template.slots.len(),
            node_count,
        ))?;
        self.next.set(next);
        self.records.borrow_mut().insert(
            id,
            Record {
                template,
                owner: owner.id(),
                owner_parent: if owner.id() == self.scope.id() {
                    None
                } else {
                    owner.parent()
                },
                parent: None,
                placed: false,
                handlers: HashMap::new(),
                bindings: Vec::new(),
                node_count,
                children: Vec::new(),
            },
        );
        self.owners.borrow_mut().insert(owner.id(), id);
        let weak = Rc::downgrade(self);
        owner
            .on_cleanup(move || {
                if let Some(state) = weak.upgrade() {
                    state.retire(id);
                }
            })
            .map_err(error)?;
        Ok(Instance {
            state: Rc::downgrade(self),
            id,
        })
    }
    fn retire(&self, id: u32) {
        let record = self.records.borrow_mut().remove(&id);
        let pending = self.pending_widgets.borrow_mut().remove(&id);
        let widget = self.widgets.borrow_mut().remove(&id);
        drop(pending);
        drop(widget);
        if let Some(record) = record {
            self.owners.borrow_mut().remove(&record.owner);
            if !self.closed.get()
                && let Err(e) = self.queue(Op::Retire(
                    id,
                    record.template.slots.len(),
                    record.node_count,
                ))
            {
                self.fail(e);
            }
            for binding in record.bindings {
                binding.dispose();
            }
            drop(record.handlers); // arbitrary captures drop outside table borrows
        }
    }
    fn flush(&self) -> Result<()> {
        self.check()?;
        if self.flushing.replace(true) {
            return Err(error("template flush already in progress"));
        }
        struct Reset<'a>(&'a Cell<bool>);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _reset = Reset(&self.flushing);
        let result = self.flush_inner();
        if let Err(e) = &result {
            self.fail(e.clone());
        }
        result
    }
    fn flush_inner(&self) -> Result<()> {
        let events = std::mem::take(&mut *self.events.borrow_mut());
        for (id, slot) in events {
            let callback = self.records.borrow().get(&id).and_then(|record| {
                record
                    .handlers
                    .get(&slot)
                    .map(|handler| (record.owner, handler.clone()))
            });
            if let Some((owner, callback)) = callback
                && owner.is_alive()
            {
                self.runtime
                    .enter(owner, || (callback.borrow_mut())())
                    .map_err(error)?;
            }
        }
        self.runtime.flush().map_err(error)?;
        if self.widgets.borrow().is_empty() && self.pending_widgets.borrow().is_empty() {
            return self.flush_host();
        }
        self.flush_host()?;
        // Only actual Widget islands enter these indices; raw-row updates
        // never scan the template instance table.
        loop {
            let mut pending: Vec<_> = self.pending_widgets.borrow_mut().drain().collect();
            if pending.is_empty() {
                break;
            }
            pending.sort_unstable_by_key(|(id, _)| *id);
            for (id, build) in pending {
                let owner = self.records.borrow().get(&id).map(|r| r.owner);
                let Some(owner) = owner.filter(|owner| owner.is_alive()) else {
                    continue;
                };
                let root = child_container(&self.host, id, 0).map_err(backend)?;
                root.set_attribute("style", "display:contents")
                    .map_err(backend)?;
                let guard = build(root, self.runtime.clone(), owner)?;
                if owner.is_alive() && self.records.borrow().contains_key(&id) {
                    self.widgets.borrow_mut().insert(id, guard);
                }
            }
            self.flush_host()?;
        }
        let mut widgets: Vec<_> = self
            .widgets
            .borrow()
            .iter()
            .map(|(&id, widget)| (id, widget.clone()))
            .collect();
        widgets.sort_unstable_by_key(|(id, _)| *id);
        for (_, widget) in widgets {
            widget.flush()?;
        }
        if !self.pending_widgets.borrow().is_empty() {
            self.bridge.wake();
        }
        self.check()?;
        self.flush_host()
    }
    fn flush_host(&self) -> Result<()> {
        self.check()?;
        let mut commands = std::mem::take(&mut *self.commands.borrow_mut());
        if commands.is_empty() {
            return Ok(());
        }
        let mut texts = std::mem::take(&mut *self.texts.borrow_mut());
        const COMMANDS_PER_PACKET: usize = 4096;
        let mut words = Vec::with_capacity(commands.len().min(COMMANDS_PER_PACKET) * 8);
        let mut strings = String::new();
        let mut offset = 0;
        while offset < commands.len() {
            words.clear();
            strings.clear();
            let mut units = 0u32;
            let mut nodes = 0;
            let first = offset;
            while offset < commands.len() && offset - first < COMMANDS_PER_PACKET {
                let mut record = [0u32; 8];
                match &commands[offset] {
                    Op::Create(id, template, _, count) => {
                        if nodes + count > 4096 {
                            break;
                        }
                        nodes += count;
                        record[..3].copy_from_slice(&[1, *id, *template])
                    }
                    Op::Write(id, slot, from, to, count) => {
                        let value = &texts[*from..*to];
                        if strings.len() + value.len() > 64 * 1024 {
                            break;
                        }
                        let start = units;
                        units += count;
                        strings.push_str(value);
                        record[..5].copy_from_slice(&[2, *id, *slot as u32, start, units]);
                    }
                    Op::Place(id, parent, before) => {
                        let (parent, slot) = parent.unwrap_or((0, 0));
                        record[..5].copy_from_slice(&[
                            3,
                            *id,
                            parent,
                            slot as u32,
                            before.unwrap_or(0),
                        ]);
                    }
                    Op::Flag(id, slot, value) => {
                        record[..4].copy_from_slice(&[5, *id, *slot as u32, u32::from(*value)])
                    }
                    Op::Retire(id, _, count) => {
                        if nodes + count > 4096 {
                            break;
                        }
                        nodes += count;
                        record[..2].copy_from_slice(&[4, *id]);
                    }
                }
                words.extend(record);
                offset += 1;
            }
            let reply = submit(&self.host, &words, &strings)
                .map_err(backend)
                .inspect_err(|e| self.fail(e.clone()))?;
            let accepted = reply
                .get(0)
                .as_f64()
                .filter(|n| n.fract() == 0. && *n >= 0. && *n <= (offset - first) as f64)
                .ok_or_else(|| error("invalid template prefix"))
                .inspect_err(|e| self.fail(e.clone()))? as usize;
            for op in &commands[first..first + accepted] {
                match op {
                    Op::Create(_, _, targets, _) => {
                        self.instances.set(self.instances.get() + 1);
                        self.targets.set(self.targets.get() + targets);
                    }
                    Op::Retire(_, targets, _) => {
                        self.instances.set(self.instances.get() - 1);
                        self.targets.set(self.targets.get() - targets);
                    }
                    _ => {}
                }
            }
            self.applied.set(self.applied.get() + accepted as u64);
            if !reply.get(1).is_null() || accepted != offset - first {
                let e = error(
                    reply
                        .get(1)
                        .as_string()
                        .unwrap_or_else(|| "invalid template completion".into()),
                );
                self.fail(e.clone());
                return Err(e);
            }
        }
        commands.clear();
        texts.clear();
        // Native hooks may enqueue new work during submit. Never overwrite it.
        if self.commands.borrow().is_empty() {
            *self.commands.borrow_mut() = commands;
        }
        if self.texts.borrow().is_empty() {
            *self.texts.borrow_mut() = texts;
        }
        Ok(())
    }
    fn close(&self) -> Result<()> {
        if self.closed.replace(true) {
            return Ok(());
        }
        self.pending.set(false);
        let result = destroy_host(&self.host).map_err(backend);
        self.scope.owner().dispose();
        let records = std::mem::take(&mut *self.records.borrow_mut());
        drop(records);
        let pending = std::mem::take(&mut *self.pending_widgets.borrow_mut());
        let widgets = std::mem::take(&mut *self.widgets.borrow_mut());
        drop(pending);
        drop(widgets);
        self.owners.borrow_mut().clear();
        drop(std::mem::take(&mut *self.commands.borrow_mut()));
        drop(std::mem::take(&mut *self.texts.borrow_mut()));
        self.templates.borrow_mut().clear();
        self.events.borrow_mut().clear();
        self.instances.set(0);
        self.targets.set(0);
        result
    }
}
impl Drop for State {
    fn drop(&mut self) {
        let _ = self.close();
        self.ingress.borrow_mut().take();
    }
}
impl Instance {
    pub(crate) fn flag_initial(&self, slot: u16) -> Result<bool> {
        let state = self.state()?;
        let records = state.records.borrow();
        match records
            .get(&self.id)
            .filter(|r| r.owner.is_alive())
            .and_then(|r| r.template.slots.get(slot as usize))
            .map(|s| s.kind)
        {
            Some(Kind::Class(_, initial)) => Ok(initial),
            _ => Err(error("not a live class slot")),
        }
    }
    pub(crate) fn set_flag(&self, slot: u16, value: bool) -> Result<()> {
        self.flag_initial(slot)?;
        self.state()?.queue(Op::Flag(self.id, slot, value))
    }
    pub(crate) fn bind_flag(
        &self,
        owner: &Scope,
        slot: u16,
        mut read: impl FnMut() -> Result<bool> + 'static,
    ) -> Result<()> {
        let mut previous = self.flag_initial(slot)?;
        let state = self.state()?;
        if !state
            .records
            .borrow()
            .get(&self.id)
            .is_some_and(|r| r.owner == owner.id() && owner.is_alive())
        {
            return Err(error("template flag owner"));
        }
        let instance = self.clone();
        let binding = owner.binding(move || {
            let next = match read() {
                Ok(next) => next,
                Err(error) => {
                    instance.fail(error);
                    return;
                }
            };
            if next != previous {
                if let Err(e) = instance.set_flag(slot, next) {
                    instance.fail(e);
                    return;
                }
                previous = next;
            }
        });
        state
            .records
            .borrow_mut()
            .get_mut(&self.id)
            .unwrap()
            .bindings
            .push(binding);
        Ok(())
    }
    pub(crate) fn fail(&self, error: Error) {
        if let Some(state) = self.state.upgrade() {
            state.fail(error);
        }
    }
    pub(crate) fn named(&self, name: &str, kind: fn(Kind) -> bool) -> Result<u16> {
        let state = self.state()?;
        let records = state.records.borrow();
        let record = records
            .get(&self.id)
            .filter(|r| r.owner.is_alive())
            .ok_or_else(|| error("dead template instance"))?;
        record
            .template
            .slot(name)
            .filter(|(_, k)| kind(*k))
            .map(|(i, _)| i)
            .ok_or_else(|| error(format!("unknown or wrong-kind slot: {name}")))
    }
    pub(crate) fn retain_child(&self, scope: Scope) -> Result<()> {
        let state = self.state()?;
        let mut records = state.records.borrow_mut();
        let record = records
            .get_mut(&self.id)
            .ok_or_else(|| error("dead template instance"))?;
        if scope.parent() != Some(record.owner) {
            return Err(error("foreign component scope"));
        }
        record.children.push(scope);
        Ok(())
    }
    fn state(&self) -> Result<Rc<State>> {
        let state = self
            .state
            .upgrade()
            .ok_or_else(|| error("template instance gone"))?;
        state.check()?;
        Ok(state)
    }
    pub fn children(&self, slot: u16) -> Result<Children> {
        let state = self.state()?;
        let valid = state.records.borrow().get(&self.id).is_some_and(|r| {
            r.owner.is_alive()
                && r.template
                    .slots
                    .get(slot as usize)
                    .is_some_and(|s| matches!(s.kind, Kind::Children | Kind::Range))
        });
        if !valid {
            return Err(error("dead instance or non-children slot"));
        }
        Ok(Children {
            instance: self.clone(),
            slot,
        })
    }
    fn validate(&self, slot: u16, kind: Kind) -> Result<Rc<State>> {
        let state = self.state()?;
        let valid = state.records.borrow().get(&self.id).is_some_and(|r| {
            r.owner.is_alive()
                && r.template
                    .slots
                    .get(slot as usize)
                    .is_some_and(|t| t.kind == kind)
        });
        if !valid {
            return Err(error("dead instance or wrong template slot"));
        }
        Ok(state)
    }
    pub fn set(&self, slot: u16, value: String) -> Result<()> {
        self.set_text(slot, &value)
    }
    fn set_text(&self, slot: u16, value: &str) -> Result<()> {
        let state = self.state()?;
        let valid = state.records.borrow().get(&self.id).is_some_and(|r| {
            r.owner.is_alive()
                && r.template
                    .slots
                    .get(slot as usize)
                    .is_some_and(|t| matches!(t.kind, Kind::Text | Kind::Attribute(_)))
        });
        if !valid {
            return Err(error("dead instance or non-writable template slot"));
        }
        state.queue_text(self.id, slot, value)
    }
    pub fn place(&self, parent: Option<&Children>, before: Option<&Instance>) -> Result<()> {
        let state = self.state()?;
        let destination = if let Some(parent) = parent {
            if !Weak::ptr_eq(&self.state, &parent.instance.state) {
                return Err(error("foreign template parent"));
            }
            parent.instance.children(parent.slot)?;
            Some((parent.instance.id, parent.slot))
        } else {
            None
        };
        let mut records = state.records.borrow_mut();
        if !records.get(&self.id).is_some_and(|r| r.owner.is_alive()) {
            return Err(error("dead template instance"));
        }
        let parent_owner = destination.and_then(|(id, _)| records.get(&id).map(|r| r.owner));
        if parent_owner != records[&self.id].owner_parent {
            return Err(error(
                "template placement must follow Scope ownership in this profile",
            ));
        }
        let mut ancestor = destination.map(|p| p.0);
        while let Some(id) = ancestor {
            if id == self.id {
                return Err(error("template cycle"));
            }
            ancestor = records.get(&id).and_then(|r| r.parent.map(|p| p.0));
        }
        let before = if let Some(before) = before {
            if !Weak::ptr_eq(&self.state, &before.state)
                || !records
                    .get(&before.id)
                    .is_some_and(|r| r.owner.is_alive() && r.placed && r.parent == destination)
            {
                return Err(error("foreign template anchor"));
            }
            Some(before.id)
        } else {
            None
        };
        state.queue(Op::Place(self.id, destination, before))?;
        let record = records.get_mut(&self.id).unwrap();
        record.parent = destination;
        record.placed = true;
        Ok(())
    }
    pub fn on_click(&self, slot: u16, callback: impl FnMut() + 'static) -> Result<()> {
        let state = self.validate(slot, Kind::Click)?;
        let previous = state
            .records
            .borrow_mut()
            .get_mut(&self.id)
            .unwrap()
            .handlers
            .insert(slot, Rc::new(RefCell::new(Box::new(callback))));
        drop(previous);
        Ok(())
    }
    /// Constant Vary values initialize once; reactive inputs create one binding.
    pub fn bind(&self, owner: &Scope, slot: u16, value: impl Into<Vary<String>>) -> Result<()> {
        let state = self.state()?;
        if !state
            .records
            .borrow()
            .get(&self.id)
            .is_some_and(|r| r.owner == owner.id() && owner.is_alive())
        {
            return Err(error("template binding owner"));
        }
        let value = value.into();
        if !value.is_reactive() {
            return self.set(slot, value.try_get().map_err(error)?);
        }
        self.bind_values(owner, [slot], move || [value.get()])
    }
    /// One pure paint region; only changed slots are emitted, without a Bond.
    pub fn bind_values<const N: usize>(
        &self,
        owner: &Scope,
        slots: [u16; N],
        mut read: impl FnMut() -> [String; N] + 'static,
    ) -> Result<()> {
        self.try_bind_values(owner, slots, move || Ok(read()))
    }
    pub(crate) fn try_bind_values<const N: usize>(
        &self,
        owner: &Scope,
        slots: [u16; N],
        mut read: impl FnMut() -> Result<[String; N]> + 'static,
    ) -> Result<()> {
        let state = self.state()?;
        let valid = state.records.borrow().get(&self.id).is_some_and(|r| {
            r.owner == owner.id()
                && owner.is_alive()
                && slots.iter().enumerate().all(|(i, slot)| {
                    !slots[..i].contains(slot)
                        && r.template
                            .slots
                            .get(*slot as usize)
                            .is_some_and(|t| matches!(t.kind, Kind::Text | Kind::Attribute(_)))
                })
        });
        if !valid {
            return Err(error("template binding owner/slot"));
        }
        let instance = self.clone();
        let mut previous: Option<[String; N]> = None;
        let binding = owner.binding(move || {
            let values = match read() {
                Ok(values) => values,
                Err(error) => {
                    instance.fail(error);
                    return;
                }
            };
            for (i, value) in values.iter().enumerate() {
                if previous.as_ref().is_none_or(|old| old[i] != *value)
                    && let Err(e) = instance.set_text(slots[i], value)
                {
                    if let Some(state) = instance.state.upgrade() {
                        state.fail(e);
                    }
                    return;
                }
            }
            previous = Some(values);
        });
        state
            .records
            .borrow_mut()
            .get_mut(&self.id)
            .unwrap()
            .bindings
            .push(binding);
        Ok(())
    }
}
