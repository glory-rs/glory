use glory_render::{memory::Memory, *};
use glory_ui::{App, WeakScope, input, view};
use std::{cell::Cell, rc::Rc};
struct Recorded {
    memory: Memory,
    disposed: Rc<Cell<usize>>,
}
impl Render for Recorded {
    fn capabilities(&self) -> Capabilities {
        self.memory.capabilities()
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let receipt = self.memory.apply(mutations)?;
        self.disposed.set(
            self.disposed.get()
                + mutations
                    .iter()
                    .filter(|mutation| matches!(mutation, Mutation::Dispose { .. }))
                    .count(),
        );
        Ok(receipt)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.memory.drain(inputs)
    }
}
#[test]
fn owned_boundary_survives_child_cleanup_until_its_owner_can_move_a_live_foreign_range() {
    let app = App::memory();
    let target_scope = app.scope().child().unwrap();
    let source = app.scope().child().unwrap();
    let target = target_scope.create(Descriptor::new(Props::View)).unwrap();
    target_scope.defer_descendant_disposal(target).unwrap();
    target_scope.defer_descendant_disposal(target).unwrap();
    let target_children = target_scope.child_in(target).unwrap();
    let child = target_children
        .create(Descriptor::new(Props::View))
        .unwrap();
    target_scope.retain(target_children).unwrap();
    let source_group = source.create(Descriptor::new(Props::Group)).unwrap();
    source.place(source_group, Some(target), None).unwrap();
    let contents = source.child_in(source_group).unwrap();
    let editor = contents
        .create(Descriptor::new(Props::Input(Default::default())))
        .unwrap();
    source.retain(contents).unwrap();
    let source_weak = source.downgrade();
    let moved = Rc::new(Cell::new(false));
    let movement = moved.clone();
    target_scope
        .on_cleanup(move || {
            let source = source_weak.upgrade().unwrap();
            source.owner().enter(|| {
                assert!(source.is_active(target).unwrap());
                assert!(source.is_active(editor).unwrap());
                assert!(
                    source.is_active(child).is_err(),
                    "ordinary owned children have already retired"
                );
                source.place(source_group, None, None).unwrap();
                movement.set(true);
            });
        })
        .unwrap();
    app.flush().unwrap();
    assert_eq!(app.stats().disposal_boundaries, 1);
    target_scope.owner().dispose();
    app.flush().unwrap();
    assert!(moved.get());
    assert!(app.tree().node(target).is_err());
    assert!(app.tree().node(editor).is_ok());
    assert_eq!(app.stats().disposal_boundaries, 0);
    source.owner().dispose();
    app.flush().unwrap();
    assert!(app.tree().is_empty());
}
#[test]
fn ordinary_bulk_disposal_keeps_coalescing_and_empty_boundary_storage() {
    let disposed = Rc::new(Cell::new(0));
    let count = disposed.clone();
    let app = App::new(move |id| Recorded {
        memory: Memory::new(id),
        disposed: count,
    });
    app.mount(view().fill((0..2000).map(|_| view().fill(input())).collect::<Vec<_>>()))
        .unwrap();
    assert_eq!(app.stats().disposal_boundaries, 0);
    app.clear().unwrap();
    assert_eq!(
        disposed.get(),
        1,
        "ordinary retired ancestors are still coalesced once"
    );
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.stats().owners, 0);
    assert_eq!(app.runtime_stats().queued, 0);
    assert_eq!(app.runtime_stats().queue_entries, 0);
}
struct Probe {
    weak: WeakScope,
    witness: NodeId,
    drops: Rc<Cell<usize>>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(scope) = self.weak.upgrade() {
            assert!(scope.is_active(self.witness).unwrap());
        }
        self.drops.set(self.drops.get() + 1);
    }
}
#[test]
fn foreign_boundaries_reject_and_terminal_cleanup_releases_captures_queries_and_markers() {
    let app = App::memory();
    let witness = app.scope().create(Descriptor::new(Props::View)).unwrap();
    let scope = app.scope().child().unwrap();
    let node = scope.create(Descriptor::new(Props::View)).unwrap();
    assert!(app.scope().defer_descendant_disposal(node).is_err());
    scope.defer_descendant_disposal(node).unwrap();
    let drops = Rc::new(Cell::new(0));
    let probe = Probe {
        weak: app.scope().downgrade(),
        witness,
        drops: drops.clone(),
    };
    scope
        .on(node, EventKind::Activate, move |_| {
            let _ = &probe;
        })
        .unwrap();
    let probe = Probe {
        weak: app.scope().downgrade(),
        witness,
        drops: drops.clone(),
    };
    scope
        .request(Query::Bounds(node), move |_| {
            let _ = &probe;
            panic!("retired query callback")
        })
        .unwrap();
    assert_eq!(app.stats().queries, 1);
    scope.owner().dispose();
    app.flush().unwrap();
    app.pump().unwrap();
    assert_eq!(drops.get(), 2);
    assert_eq!(app.stats().handlers, 0);
    assert_eq!(app.stats().queries, 0);
    assert_eq!(app.stats().query_owners, 0);
    assert_eq!(app.stats().query_targets, 0);
    assert_eq!(app.stats().disposal_boundaries, 0);
    assert!(app.scope().defer_descendant_disposal(node).is_err());
    let other = App::memory();
    assert!(other.scope().defer_descendant_disposal(witness).is_err());
}
