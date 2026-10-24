//! Integrate real Owner/Binding/Task lifetimes with a lazily indexed million-row
//! window. This still uses the semantic interpreter, not a DOM/browser host.
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use glory_core::{Cage, Runtime, Scope, TaskToken};
use glory_render::memory::Memory;
use glory_render::{
    Change, Descriptor, Mutation, NodeId, NodeIds, Patch, Props, Render, TextProps,
};
use glory_ui::list::{Cache, Heights, Layout, RowHost};

struct Row {
    id: NodeId,
    scope: Scope,
    index: Cage<usize>,
    task: TaskToken,
}

struct Host {
    runtime: Runtime,
    root: Scope,
    ids: NodeIds,
    root_id: NodeId,
    memory: Rc<RefCell<Memory>>,
    edits: Cage<BTreeMap<usize, String>>,
    cancelled: Rc<Cell<usize>>,
}

impl Host {
    fn new() -> Self {
        let runtime = Runtime::new();
        let root = runtime.scope();
        let edits = root.cage(BTreeMap::new());
        let mut ids = NodeIds::new(runtime.id());
        let root_id = ids.allocate();
        let memory = Rc::new(RefCell::new(Memory::new(runtime.id())));
        memory
            .borrow_mut()
            .apply(&[
                Mutation::Create {
                    id: root_id,
                    node: Box::new(Descriptor::new(Props::View)),
                },
                Mutation::Place {
                    id: root_id,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
        Self {
            runtime,
            root,
            ids,
            root_id,
            memory,
            edits,
            cancelled: Rc::new(Cell::new(0)),
        }
    }

    fn text(&self, row: &Row) -> String {
        let memory = self.memory.borrow();
        let Props::Text(props) = &memory.node(row.id).unwrap().descriptor.view().props else {
            panic!()
        };
        props.content.clone()
    }
}

impl RowHost for Host {
    type Key = usize;
    type Row = Row;
    fn key(&mut self, index: usize) -> usize {
        index
    }
    fn mount(&mut self, index: usize, key: &usize) -> Row {
        let scope = self.root.child();
        let row_index = scope.cage(index);
        let id = self.ids.allocate();
        self.memory
            .borrow_mut()
            .apply(&[Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Text(TextProps::default()))),
            }])
            .unwrap();
        let key = *key;
        let edits = self.edits;
        // The global data stays borrowed. Only one row's label is cloned.
        let label = scope
            .bond(move || {
                edits.with(|edits| {
                    edits
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| format!("row {key}"))
                })
            })
            .with_partial_eq();
        let memory = self.memory.clone();
        scope.binding(move || {
            memory
                .borrow_mut()
                .apply(&[Mutation::Patch {
                    id,
                    patch: Patch::Text(Change::Set(label.get())),
                }])
                .unwrap();
        });
        let task = scope.task();
        let cancelled = self.cancelled.clone();
        task.on_cancel(move || cancelled.set(cancelled.get() + 1))
            .unwrap();
        Row {
            id,
            scope,
            index: row_index,
            task: task.detach(),
        }
    }
    fn update(&mut self, index: usize, _: &usize, row: &mut Row) {
        if row.index.get_untracked() != index {
            row.index.revise(|value| *value = index);
        }
    }
    fn place(&mut self, row: &Row, before: Option<&Row>) {
        self.memory
            .borrow_mut()
            .apply(&[Mutation::Place {
                id: row.id,
                parent: Some(self.root_id),
                before: before.map(|row| row.id),
            }])
            .unwrap();
    }
    fn dispose(&mut self, row: Row) {
        self.memory
            .borrow_mut()
            .apply(&[Mutation::Dispose { id: row.id }])
            .unwrap();
        assert!(self.ids.release(row.id));
        drop(row);
    }
}

#[test]
fn visible_rows_have_owned_reactive_bindings_and_only_changed_labels_patch() {
    let mut host = Host::new();
    let layout = Layout::new(Heights::fixed(1_000_000, 24.0).unwrap(), 4);
    let window = layout.window(12_000_000.0, 600.0).unwrap();
    let mut cache = Cache::default();
    cache.reconcile(window.render, 1, &mut host).unwrap();
    host.runtime.flush().unwrap();
    assert_eq!(cache.len(), 33);
    let row = cache.row(&500_000).unwrap();
    assert_eq!(host.text(row), "row 500000");
    assert_eq!(row.scope.parent(), Some(host.root.id()));
    let task = row.task;
    let id = row.id;
    let stats = host.runtime.stats();
    let patches = host.memory.borrow().stats().patched;
    host.edits.revise(|edits| {
        edits.insert(500_000, String::from("edited"));
    });
    host.runtime.flush().unwrap();
    assert_eq!(host.runtime.stats().binding_runs, stats.binding_runs + 1);
    assert_eq!(host.memory.borrow().stats().patched, patches + 1);
    assert_eq!(host.text(cache.row(&500_000).unwrap()), "edited");
    let next = layout.window(0.0, 600.0).unwrap();
    cache.reconcile(next.render, 1, &mut host).unwrap();
    host.runtime.flush().unwrap();
    assert!(!host.ids.contains(id));
    assert!(!task.is_current());
    assert_eq!(host.cancelled.get(), 33);
    cache
        .reconcile(
            layout.window(12_000_000.0, 600.0).unwrap().render,
            1,
            &mut host,
        )
        .unwrap();
    host.runtime.flush().unwrap();
    assert_ne!(cache.row(&500_000).unwrap().id, id);
    assert_eq!(host.text(cache.row(&500_000).unwrap()), "edited");
    cache.clear(&mut host);
    host.runtime.flush().unwrap();
    let stats = host.runtime.stats();
    assert_eq!(
        (stats.nodes, stats.edges, stats.tasks, stats.instances),
        (1, 0, 0, 1)
    );
    assert_eq!(host.memory.borrow().len(), 1);
}

#[test]
fn repeated_far_scrolls_bound_real_graph_slots_and_cancel_offscreen_work() {
    let mut host = Host::new();
    let mut cache = Cache::default();
    let layout = Layout::new(Heights::fixed(1_000_000, 24.0).unwrap(), 4);
    let mut created = 0;
    for index in (0..1_000_000).step_by(1003) {
        let result = cache
            .reconcile(
                layout.window(index as f64 * 24.0, 600.0).unwrap().render,
                1,
                &mut host,
            )
            .unwrap();
        created += result.created;
        host.runtime.flush().unwrap();
        let stats = host.runtime.stats();
        assert!(stats.nodes <= 100 && stats.node_slots <= 100);
        assert!(stats.owners <= 35 && stats.owner_slots <= 35);
        assert!(stats.instances <= 34 && stats.instance_slots <= 34);
        assert!(stats.tasks <= 33 && stats.task_slots <= 33);
        assert!(stats.edges <= 66);
        assert_eq!(host.cancelled.get(), created - cache.len());
        assert_eq!(host.memory.borrow().len(), cache.len() + 1);
    }
    cache.clear(&mut host);
    assert_eq!(host.cancelled.get(), created);
    assert_eq!(host.runtime.stats().nodes, 1);
}
