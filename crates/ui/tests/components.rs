use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory_core::Cage;
use glory_render::memory::Memory;
use glory_render::{
    Capabilities, Error as RenderError, Event, Input as RenderInput, Mutation, NodeId, Props,
    Query, QueryToken, Receipt, Render, RuntimeId, TextKind,
};
use glory_ui::events::*;
use glory_ui::*;

#[test]
fn focus_observations_cross_groups_without_stealing_direct_control_events() {
    let app = App::memory();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let direct = Rc::new(Cell::new(0));
    let outer = seen.clone();
    let inner = seen.clone();
    let input_calls = direct.clone();
    app.mount(
        view()
            .on(focus_within, move |event| {
                outer.borrow_mut().push(("outer", event))
            })
            .fill(component(move |scope| {
                let range = group()
                    .fill(input().on(focus_changed, {
                        let calls = input_calls.clone();
                        move |_| calls.set(calls.get() + 1)
                    }))
                    .mount(scope)?;
                let inner = inner.clone();
                scope.on(range.roots()[0], focus_within, move |event| {
                    inner.borrow_mut().push(("group", event))
                })?;
                scope.retain(range)
            })),
    )
    .unwrap();
    let target = find(&app, |props| matches!(props, Props::Input(_)));
    app.dispatch(target, Event::FocusChanged(true)).unwrap();
    app.dispatch(target, Event::FocusChanged(false)).unwrap();
    assert_eq!(direct.get(), 2);
    assert_eq!(
        *seen.borrow(),
        vec![
            (
                "group",
                Event::FocusWithin {
                    node: target,
                    focused: true
                }
            ),
            (
                "outer",
                Event::FocusWithin {
                    node: target,
                    focused: true
                }
            ),
            (
                "group",
                Event::FocusWithin {
                    node: target,
                    focused: false
                }
            ),
            (
                "outer",
                Event::FocusWithin {
                    node: target,
                    focused: false
                }
            ),
        ]
    );
    app.clear().unwrap();
    assert!(!app.dispatch(target, Event::FocusChanged(true)).unwrap());
}

#[test]
fn focus_callbacks_do_not_call_an_ancestor_observer_disposed_by_the_origin() {
    let app = App::memory();
    let observers = app.scope().child().unwrap();
    let cancel = observers.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    app.mount(view().fill(input().on(focus_changed, move |_| cancel.dispose())))
        .unwrap();
    let root = app.tree().roots()[0];
    observers
        .on(root, focus_within, move |_| {
            observed.set(observed.get() + 1)
        })
        .unwrap();
    let target = find(&app, |props| matches!(props, Props::Input(_)));
    app.dispatch(target, Event::FocusChanged(true)).unwrap();
    assert_eq!(calls.get(), 0);
}

#[test]
fn suspended_live_scopes_receive_terminal_focus_and_composition_loss_but_disposed_scopes_do_not() {
    let app = App::memory();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let direct = seen.clone();
    let within = seen.clone();
    let compositions = seen.clone();
    let mounted = view()
        .on(focus_within, move |event| within.borrow_mut().push(event))
        .fill(
            input()
                .on(focus_changed, move |event| direct.borrow_mut().push(event))
                .on(composition, move |event| {
                    compositions.borrow_mut().push(event)
                }),
        )
        .mount(app.scope())
        .unwrap();
    app.flush().unwrap();
    let root = mounted.roots()[0];
    let target = app.tree().node(root).unwrap().children[0];
    assert!(app.dispatch(target, Event::FocusChanged(true)).unwrap());
    seen.borrow_mut().clear();
    mounted.suspend().unwrap();
    app.flush().unwrap();
    assert!(!app.dispatch(target, Event::Activate).unwrap());
    assert!(!app.dispatch(target, Event::FocusChanged(true)).unwrap());
    assert!(app.dispatch(target, Event::FocusChanged(false)).unwrap());
    assert!(
        !app.dispatch(target, Event::Composition { active: true })
            .unwrap()
    );
    assert!(
        app.dispatch(target, Event::Composition { active: false })
            .unwrap()
    );
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            Event::FocusChanged(false),
            Event::FocusWithin {
                node: target,
                focused: false
            },
            Event::Composition { active: false }
        ]
    );
    drop(mounted);
    app.flush().unwrap();
    assert!(!app.dispatch(target, Event::FocusChanged(false)).unwrap());
    assert!(
        !app.dispatch(target, Event::Composition { active: false })
            .unwrap()
    );
    assert_eq!(seen.borrow().len(), 3);
}

#[derive(Default)]
struct Trace {
    mutations: Vec<Mutation>,
    queries: Vec<String>,
}
struct Recorder {
    memory: Memory,
    trace: Rc<RefCell<Trace>>,
    delayed: bool,
}
impl Render for Recorder {
    fn style_profile(&self) -> glory_render::style::Profile {
        self.memory.style_profile()
    }
    fn capabilities(&self) -> Capabilities {
        self.memory.capabilities()
    }
    fn apply(&mut self, mutations: &[Mutation]) -> std::result::Result<Receipt, RenderError> {
        self.trace
            .borrow_mut()
            .mutations
            .extend_from_slice(mutations);
        self.memory.apply(mutations)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> std::result::Result<(), RenderError> {
        self.trace.borrow_mut().queries.push(format!(
            "{:?}",
            self.memory.node(query.node())?.descriptor.props
        ));
        if self.delayed {
            Ok(())
        } else {
            self.memory.request(query, token)
        }
    }
    fn drain(&mut self, inputs: &mut Vec<RenderInput>) {
        self.memory.drain(inputs);
    }
}
fn app(delayed: bool) -> (App, Rc<RefCell<Trace>>) {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let copy = trace.clone();
    (
        App::new(move |id: RuntimeId| Recorder {
            memory: Memory::new(id),
            trace: copy,
            delayed,
        }),
        trace,
    )
}
fn find(app: &App, test: impl Fn(&Props) -> bool) -> NodeId {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        if test(&node.descriptor.props) {
            return id;
        }
        pending.extend(node.children.iter().copied());
    }
    panic!("node not found")
}
fn text_at(app: &App, id: NodeId) -> String {
    let tree = app.tree();
    let Props::Text(text) = &tree.node(id).unwrap().descriptor.props else {
        panic!()
    };
    text.content.clone()
}

#[test]
fn counter_uses_public_builders_and_one_patch_per_event_tick() {
    let (app, trace) = app(false);
    let count = app.scope().cage(0);
    let label = count.map(|value| value.to_string());
    let ui = view().class("counter").fill((
        text().content(label),
        button().label("add").on(activate, move |_| {
            count.revise(|n| *n += 1);
            count.revise(|n| *n += 1);
        }),
    ));
    assert_eq!(app.stats().nodes, 0); // Builder construction creates no nodes.
    app.mount(ui).unwrap();
    let text = find(&app, |props| matches!(props, Props::Text(_)));
    let button = find(&app, |props| matches!(props, Props::Button(_)));
    assert_eq!(text_at(&app, text), "0");
    trace.borrow_mut().mutations.clear();
    app.dispatch(button, Event::Activate).unwrap();
    assert_eq!(text_at(&app, text), "2");
    assert_eq!(trace.borrow().mutations.len(), 1);
    assert!(matches!(trace.borrow().mutations[0], Mutation::Patch { id, .. } if id == text));
    app.clear().unwrap();
    assert_eq!(
        (app.stats().nodes, app.stats().handlers, app.stats().owners),
        (0, 0, 0)
    );
    assert!(!app.dispatch(button, Event::Activate).unwrap());
    assert_eq!(count.get(), 2);
}

#[test]
fn only_the_last_binding_for_a_property_is_installed() {
    let app = App::memory();
    let a = app.scope().cage(String::from("a"));
    let b = app.scope().cage(String::from("b"));
    app.mount(text().content(a).content(b)).unwrap();
    assert_eq!(app.runtime_stats().edges, 1);
    let id = find(&app, |_| true);
    a.revise(|value| *value = "unused".into());
    app.flush().unwrap();
    assert_eq!(text_at(&app, id), "b");
    b.revise(|value| *value = "changed".into());
    app.flush().unwrap();
    assert_eq!(text_at(&app, id), "changed");
}

#[test]
fn eight_components_and_configuration_validation_are_backend_independent() {
    let app = App::memory();
    app.mount(
        view().fill((
            group().fill(text().content("heading").kind(TextKind::Heading(2))),
            button().label("submit"),
            input()
                .purpose(Purpose::Password)
                .multiline(true)
                .purpose(Purpose::Text)
                .value("body"),
            check().checked(false).label("agree"),
            image().source("photo.webp").alt("photo"),
            link().target("/about").label("about"),
        )),
    )
    .unwrap();
    assert_eq!(app.stats().nodes, 8);
    app.clear().unwrap();
    assert!(app.mount(image().source("missing-alt.webp")).is_err());
    assert_eq!(app.stats().nodes, 0);
    assert!(app.mount(text().kind(TextKind::Heading(9))).is_err());
    assert_eq!(app.stats().nodes, 0);
    assert!(app.mount(button().fill(button().label("nested"))).is_err());
    assert_eq!(app.stats().nodes, 0);
    app.mount(image().source("decorative.webp").decorative())
        .unwrap();
}

#[test]
fn structural_phases_precede_props_and_each_reuses_row_state_on_reorder() {
    #[derive(Clone)]
    struct Item {
        id: u64,
        label: String,
    }
    let (app, trace) = app(false);
    let items = app.scope().cage(vec![
        Item {
            id: 1,
            label: "one".into(),
        },
        Item {
            id: 2,
            label: "two".into(),
        },
        Item {
            id: 3,
            label: "three".into(),
        },
    ]);
    let made = Rc::new(Cell::new(0));
    let calls = made.clone();
    let counters = Rc::new(RefCell::new(
        std::collections::HashMap::<u64, Cage<usize>>::new(),
    ));
    let state = counters.clone();
    app.mount(each(
        items,
        |item| item.id,
        move |item, index| {
            calls.set(calls.get() + 1);
            let state = state.clone();
            component(move |scope| {
                let count = scope.cage(0);
                state.borrow_mut().insert(item.with(|item| item.id), count);
                view()
                    .fill((
                        text().content(item.map(|item| item.label.clone())),
                        text().content(index.map(|index| format!("index:{index}"))),
                        button()
                            .label("row")
                            .on(activate, move |_| count.revise(|v| *v += 1)),
                    ))
                    .show_in(scope)
            })
        },
    ))
    .unwrap();
    let one = find(
        &app,
        |props| matches!(props, Props::Text(text) if text.content == "one"),
    );
    let count = counters.borrow()[&1];
    count.revise(|value| *value = 7);
    trace.borrow_mut().mutations.clear();
    items.revise(|items| items.rotate_left(1));
    app.flush().unwrap();
    assert_eq!(made.get(), 3);
    assert_eq!(counters.borrow()[&1].get(), 7);
    assert_eq!(text_at(&app, one), "one");
    assert_eq!(
        trace
            .borrow()
            .mutations
            .iter()
            .filter(|mutation| matches!(mutation, Mutation::Place { .. }))
            .count(),
        1
    );
    items.revise(|items| items[2].label = "updated".into());
    app.flush().unwrap();
    assert_eq!(text_at(&app, one), "updated");
    assert_eq!(made.get(), 3);
    items.revise(|items| {
        items.remove(2);
    });
    app.flush().unwrap();
    assert!(!count.is_alive());
}

#[test]
fn when_does_not_subscribe_to_its_child_props_and_reclaims_every_branch() {
    let app = App::memory();
    let show = app.scope().cage(true);
    let label = app.scope().cage(String::from("first"));
    let calls = Rc::new(Cell::new(0));
    let builds = calls.clone();
    let cancelled = Rc::new(Cell::new(0));
    let aborts = cancelled.clone();
    app.mount(when(show, move || {
        builds.set(builds.get() + 1);
        let aborts = aborts.clone();
        component(move |scope| {
            let task = scope.task();
            let aborts = aborts.clone();
            task.on_cancel(move || aborts.set(aborts.get() + 1))?;
            task.detach();
            text().content(label).show_in(scope)
        })
    }))
    .unwrap();
    label.revise(|value| *value = "next".into());
    app.flush().unwrap();
    assert_eq!(calls.get(), 1);
    for index in 0..500 {
        show.revise(|show| *show = false);
        app.flush().unwrap();
        assert_eq!(app.stats().nodes, 1);
        assert_eq!(app.runtime_stats().tasks, 0);
        show.revise(|show| *show = true);
        app.flush().unwrap();
        assert_eq!(app.stats().nodes, 2);
        assert_eq!(app.runtime_stats().tasks, 1);
        assert!(app.stats().node_slots <= 2);
        assert!(app.runtime_stats().owner_slots <= 8);
        assert_eq!(cancelled.get(), index + 1);
    }
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.runtime_stats().edges, 0);
}

#[test]
fn activation_from_child_text_targets_nearest_handler_and_respects_disabled_ancestors() {
    let app = App::memory();
    let enabled = app.scope().cage(true);
    let clicks = app.scope().cage(0);
    app.mount(
        button()
            .enabled(enabled)
            .fill(text().content("click here"))
            .on(activate, move |_| clicks.revise(|value| *value += 1)),
    )
    .unwrap();
    let child = find(&app, |props| matches!(props, Props::Text(_)));
    assert!(app.dispatch(child, Event::Activate).unwrap());
    assert_eq!(clicks.get(), 1);
    enabled.revise(|value| *value = false);
    app.flush().unwrap();
    assert!(!app.dispatch(child, Event::Activate).unwrap());
    assert_eq!(clicks.get(), 1);
}

#[test]
fn input_revisions_and_control_event_types_are_checked_before_user_callbacks() {
    let app = App::memory();
    let value = app.scope().cage(String::from("initial"));
    let edits = app.scope().cage(0);
    app.mount(input().value(value).on(value_changed, move |event| {
        if let Event::ValueChanged { value: text, .. } = event {
            edits.revise(|n| *n += 1);
            value.revise(|v| *v = text);
        }
    }))
    .unwrap();
    let id = find(&app, |_| true);
    assert!(
        !app.dispatch(
            id,
            Event::ValueChanged {
                value: "old".into(),
                composing: false,
                revision: 0
            }
        )
        .unwrap()
    );
    assert!(
        app.dispatch(
            id,
            Event::ValueChanged {
                value: "edited".into(),
                composing: true,
                revision: 1
            }
        )
        .unwrap()
    );
    assert_eq!(edits.get(), 1);
    assert_eq!(value.get(), "edited");
    assert!(app.dispatch(id, Event::CheckedChanged(true)).is_err());
}

#[test]
fn queries_flush_before_request_and_disposed_waiters_complete_exactly_once() {
    let (app, trace) = app(true);
    let label = app.scope().cage(String::from("A"));
    app.mount(text().content(label)).unwrap();
    let id = find(&app, |_| true);
    label.revise(|value| *value = "before query".into());
    let replies = Rc::new(RefCell::new(Vec::new()));
    let received = replies.clone();
    app.request(Query::Bounds(id), move |reply| {
        received.borrow_mut().push(reply)
    })
    .unwrap();
    assert!(trace.borrow().queries[0].contains("before query"));
    assert_eq!(app.stats().queries, 1);
    app.clear().unwrap();
    assert!(matches!(&replies.borrow()[..], [Err(RenderError::NodeGone(node))] if *node == id));
    assert_eq!(app.stats().queries, 0);
    app.pump().unwrap();
    assert_eq!(replies.borrow().len(), 1);
}

#[test]
fn app_drop_invalidates_external_scope_clones_and_drops_resources() {
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let app = App::memory();
    let root = app.scope().clone();
    let disposed = Rc::new(Cell::new(false));
    root.retain(Dropped(disposed.clone())).unwrap();
    let value = root.cage(1);
    app.mount(text().content("body")).unwrap();
    drop(app);
    assert!(!root.is_alive());
    assert!(!value.is_alive());
    assert!(disposed.get());
    assert!(root.child().is_err());
}

#[test]
fn keyed_empty_and_multi_root_components_keep_rendered_order() {
    let app = App::memory();
    let source = app.scope().cage(vec![1_i32, 0, 2]);
    app.mount(each(
        source,
        |value| *value,
        |item, _| {
            component(move |scope| {
                if item.get() != 0 {
                    text()
                        .content(item.map(|item| format!("{item}:a")))
                        .show_in(scope)?;
                    text()
                        .content(item.map(|item| format!("{item}:b")))
                        .show_in(scope)?;
                }
                Ok(())
            })
        },
    ))
    .unwrap();
    let labels = || {
        let tree = app.tree();
        let parent = tree.roots()[0];
        tree.node(parent)
            .unwrap()
            .children
            .iter()
            .map(|id| {
                let Props::Text(text) = &tree.node(*id).unwrap().descriptor.props else {
                    panic!()
                };
                text.content.clone()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(), ["1:a", "1:b", "2:a", "2:b"]);
    source.revise(|items| items.reverse());
    app.flush().unwrap();
    assert_eq!(labels(), ["2:a", "2:b", "1:a", "1:b"]);
}

#[test]
fn query_cancellation_callbacks_settle_their_state_in_the_same_flush() {
    let (app, _) = app(true);
    let finished = app.scope().cage(false);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let results = seen.clone();
    app.scope()
        .binding(move || results.borrow_mut().push(finished.get()));
    app.mount(text().content("query target")).unwrap();
    let id = find(&app, |_| true);
    app.request(Query::Bounds(id), move |_| {
        finished.revise(|value| *value = true)
    })
    .unwrap();
    app.clear().unwrap();
    assert_eq!(&*seen.borrow(), &[false, true]);
}

#[test]
fn repeated_root_and_branch_factories_release_maps_of_application_sources() {
    let app = App::memory();
    let source = app.scope().cage(1);
    for _ in 0..100 {
        app.mount_with(|| text().content(source.map(|value| value.to_string())))
            .unwrap();
        app.clear().unwrap();
        assert_eq!(app.runtime_stats().nodes, 1);
    }
    let show = app.scope().cage(true);
    app.mount(when(show, move || {
        text().content(source.map(|value| value.to_string()))
    }))
    .unwrap();
    for _ in 0..100 {
        show.revise(|value| *value = false);
        app.flush().unwrap();
        assert_eq!(app.runtime_stats().nodes, 3);
        show.revise(|value| *value = true);
        app.flush().unwrap();
        assert_eq!(app.runtime_stats().nodes, 5);
    }
}

#[test]
fn failed_root_factories_do_not_accumulate_pending_create_dispose_history() {
    let app = App::memory();
    for _ in 0..100 {
        assert!(
            app.mount_with(|| button().fill(button().label("invalid")))
                .is_err()
        );
        assert_eq!(app.stats().nodes, 0);
        assert_eq!(app.stats().pending, 0);
        assert!(app.stats().node_slots <= 2);
    }
    assert!(app.mount(input().value("not\na single line")).is_err());
    app.mount(input().multiline(true).value("two\nlines"))
        .unwrap();
}

#[test]
fn listeners_on_a_persistent_target_belong_to_the_subscribing_scope() {
    let app = App::memory();
    app.mount(button().label("persistent")).unwrap();
    let target = find(&app, |_| true);
    let calls = Rc::new(Cell::new(0));
    for _ in 0..100 {
        let subscriber = app.scope().child().unwrap();
        let copy = calls.clone();
        subscriber
            .on(target, activate, move |_| copy.set(copy.get() + 1))
            .unwrap();
        app.flush().unwrap();
        assert!(app.dispatch(target, Event::Activate).unwrap());
        subscriber.dispose();
        app.flush().unwrap();
        assert_eq!(app.stats().handlers, 0);
        assert!(!app.dispatch(target, Event::Activate).unwrap());
    }
    assert_eq!(calls.get(), 100);
    let older = app.scope().child().unwrap();
    let newer = app.scope().child().unwrap();
    older
        .on(target, activate, |_| panic!("replaced handler"))
        .unwrap();
    let copy = calls.clone();
    newer
        .on(target, activate, move |_| copy.set(copy.get() + 1))
        .unwrap();
    older.dispose();
    assert!(app.dispatch(target, Event::Activate).unwrap());
    assert_eq!(calls.get(), 101);
    newer.dispose();
    app.flush().unwrap();
    assert_eq!(app.stats().handlers, 0);
}
