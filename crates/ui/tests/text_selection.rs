use glory_render::memory::Memory;
use glory_render::*;
use glory_ui::{App, input};
use std::{cell::RefCell, rc::Rc};
struct Capture {
    tree: Memory,
    actions: Rc<RefCell<Vec<Action>>>,
}
impl Render for Capture {
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.tree.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.tree.drain(inputs)
    }
    fn apply(&mut self, m: &[Mutation]) -> Result<Receipt, Error> {
        self.tree.apply(m)
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            input: true,
            ..Default::default()
        }
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        action.validate_target(&self.tree.node(action.node())?.descriptor)?;
        self.actions.borrow_mut().push(action);
        Ok(())
    }
}
#[test]
fn owner_identity_and_current_revision_validate_before_queue_and_superseded_selection_is_not_applied()
 {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let out = actions.clone();
    let app = App::new(move |runtime| Capture {
        tree: Memory::new(runtime),
        actions: out,
    });
    let r = app.scope().cage(None);
    app.mount(input().value("000001").node_ref(r)).unwrap();
    let id = r.get().unwrap();
    app.scope()
        .set_input_selection(
            id,
            TextSelection {
                start: 1,
                end: 3,
                direction: SelectionDirection::Backward,
            },
        )
        .unwrap();
    app.scope().set_input_value(id, "000002").unwrap();
    app.flush().unwrap();
    assert!(actions.borrow().is_empty());
    app.scope()
        .set_input_selection(
            id,
            TextSelection {
                start: 2,
                end: 4,
                direction: SelectionDirection::Forward,
            },
        )
        .unwrap();
    app.flush().unwrap();
    assert_eq!(actions.borrow().len(), 1);
    let before = app.stats();
    assert!(
        app.scope()
            .action(Action::SelectText {
                node: id,
                revision: 0,
                selection: TextSelection::default()
            })
            .is_err()
    );
    assert_eq!(app.stats(), before);
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert!(
        app.scope()
            .set_input_selection(foreign, TextSelection::default())
            .is_err()
    );
    app.clear().unwrap();
    assert!(
        app.scope()
            .set_input_selection(id, TextSelection::default())
            .is_err()
    );
}
#[test]
fn reference_host_refuses_selection_before_queue_without_a_real_input_driver() {
    let app = App::memory();
    let r = app.scope().cage(None);
    app.mount(input().value("000001").node_ref(r)).unwrap();
    let before = app.stats();
    assert!(
        app.scope()
            .set_input_selection(r.get().unwrap(), TextSelection::default())
            .is_err()
    );
    assert_eq!(app.stats(), before);
}

#[test]
fn unicode_current_value_selection_rejects_half_surrogates_before_queue_and_supersedes_old_intents()
{
    let actions = Rc::new(RefCell::new(Vec::new()));
    let out = actions.clone();
    let app = App::new(move |runtime| Capture {
        tree: Memory::new(runtime),
        actions: out,
    });
    let node = app.scope().cage(None);
    app.mount(input().value("A中😀Z").node_ref(node)).unwrap();
    let id = node.get().unwrap();
    let before = app.stats();
    assert!(
        app.scope()
            .set_input_selection(
                id,
                TextSelection {
                    start: 3,
                    end: 3,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(app.stats(), before);
    app.scope()
        .set_input_selection(
            id,
            TextSelection {
                start: 1,
                end: 4,
                direction: SelectionDirection::Backward,
            },
        )
        .unwrap();
    app.flush().unwrap();
    assert!(matches!(
        actions.borrow().as_slice(),
        [Action::SelectText {
            selection: TextSelection {
                start: 1,
                end: 4,
                direction: SelectionDirection::Backward
            },
            ..
        }]
    ));
    actions.borrow_mut().clear();
    app.scope()
        .set_input_selection(id, TextSelection::default())
        .unwrap();
    app.scope().set_input_value(id, "另一😀值").unwrap();
    app.flush().unwrap();
    assert!(actions.borrow().is_empty());
}

#[test]
fn readonly_opted_in_edit_does_not_reach_generic_handlers_but_composition_cleanup_does() {
    let app = App::memory();
    let r = app.scope().cage(None);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let a = calls.clone();
    let b = calls.clone();
    app.mount(
        input()
            .value("00")
            .node_ref(r)
            .semantics(Some(Box::new(Semantics {
                readonly: true,
                input_edit: Some(InputEditCapture { max_bytes: 8 }),
                ..Default::default()
            })))
            .on(EventKind::InputEdit, move |_| a.borrow_mut().push("edit"))
            .on(EventKind::Composition, move |_| {
                b.borrow_mut().push("cleanup")
            }),
    )
    .unwrap();
    let id = r.get().unwrap();
    let revision = {
        let tree = app.tree();
        let Props::Input(p) = tree.node(id).unwrap().descriptor.view().props else {
            panic!()
        };
        p.revision
    };
    app.dispatch(
        id,
        Event::InputEdit(InputEdit::Rejected {
            revision,
            reason: InputEditRejection::TooLong,
        }),
    )
    .unwrap();
    assert!(calls.borrow().is_empty());
    app.dispatch(id, Event::Composition { active: false })
        .unwrap();
    assert_eq!(calls.borrow().as_slice(), ["cleanup"]);
}

#[test]
fn opted_in_edit_revisions_drop_old_inputs_reject_future_and_retire_with_the_owner() {
    let app = App::memory();
    let r = app.scope().cage(None);
    let calls = Rc::new(RefCell::new(0));
    let out = calls.clone();
    app.mount(
        input()
            .value("00")
            .node_ref(r)
            .semantics(Some(Box::new(Semantics {
                input_edit: Some(InputEditCapture { max_bytes: 8 }),
                ..Default::default()
            })))
            .on(EventKind::InputEdit, move |_| *out.borrow_mut() += 1),
    )
    .unwrap();
    let id = r.get().unwrap();
    let event = |revision| {
        Event::InputEdit(InputEdit::Value {
            value: "1".into(),
            revision,
            selection: TextSelection {
                start: 1,
                end: 1,
                ..Default::default()
            },
            before: None,
            intent: InputEditIntent::Replace,
            composing: false,
        })
    };
    let initial_revision = {
        let tree = app.tree();
        let Props::Input(p) = tree.node(id).unwrap().descriptor.view().props else {
            panic!()
        };
        p.revision
    };
    app.dispatch(id, event(initial_revision)).unwrap();
    assert_eq!(*calls.borrow(), 1);
    app.scope().set_input_value(id, "000").unwrap();
    app.flush().unwrap();
    assert!(!app.dispatch(id, event(initial_revision)).unwrap());
    assert_eq!(*calls.borrow(), 1);
    assert!(app.dispatch(id, event(initial_revision + 2)).is_err());
    assert_eq!(*calls.borrow(), 1);
    app.dispatch(
        id,
        Event::InputEdit(InputEdit::Rejected {
            revision: initial_revision + 1,
            reason: InputEditRejection::TooLong,
        }),
    )
    .unwrap();
    assert_eq!(*calls.borrow(), 2);
    app.clear().unwrap();
    assert!(!app.dispatch(id, event(initial_revision + 1)).unwrap());
    assert_eq!(*calls.borrow(), 2);
}
