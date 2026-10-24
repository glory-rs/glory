use glory_render::memory::Memory;
use glory_render::*;
use glory_ui::{App, Widget, button, input, view};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn repeated_event_path_selection_and_derived_values_belong_to_the_mounted_listener() {
    let app = App::memory();
    let model = app.scope().cage((0usize,));
    let saved = Rc::new(RefCell::new(
        None::<(glory_core::Cage<usize>, glory_core::Bond<usize>)>,
    ));
    let out = saved.clone();
    let mounted = button()
        .on(EventKind::Activate, move |_| {
            let field = model.field("count", |value| &value.0, |value| &mut value.0);
            if out.borrow().is_none() {
                *out.borrow_mut() = Some((field, model.map(|value| value.0)));
            }
            field.revise(|value| *value += 1);
        })
        .mount(app.scope())
        .unwrap();
    let node = mounted.roots()[0];
    for count in 1..=1000 {
        assert!(app.dispatch(node, Event::Activate).unwrap());
        let (field, derived) = saved.borrow().unwrap();
        assert_eq!((field.get(), derived.get()), (count, count));
        assert_eq!(app.runtime_stats().nodes, 3);
    }
    drop(mounted);
    app.flush().unwrap();
    let (field, derived) = saved.borrow().unwrap();
    assert!(!field.is_alive() && !derived.is_alive());
    assert!(model.is_alive());
    assert_eq!(
        (app.runtime_stats().nodes, app.runtime_stats().fields),
        (1, 0)
    );
    assert!(!app.dispatch(node, Event::Activate).unwrap());
}

#[test]
fn event_allocations_follow_the_registering_scope_even_on_another_scopes_node() {
    let app = App::memory();
    let model = app.scope().cage((4usize,));
    let mounted = button().mount(app.scope()).unwrap();
    let node = mounted.roots()[0];
    let mut previous = None;
    for _ in 0..1000 {
        let owner = app.scope().child().unwrap();
        let saved = Rc::new(Cell::new(None));
        let out = saved.clone();
        owner
            .on(node, EventKind::Activate, move |_| {
                out.set(Some(model.field("value", |v| &v.0, |v| &mut v.0)))
            })
            .unwrap();
        assert!(app.dispatch(node, Event::Activate).unwrap());
        let alias = saved.get().unwrap();
        assert_eq!(alias.get(), 4);
        owner.dispose();
        app.flush().unwrap();
        assert!(!alias.is_alive());
        if let Some(previous) = previous {
            assert!(!glory_core::Cage::is_alive(previous));
        }
        previous = Some(alias);
        assert_eq!(
            (app.runtime_stats().nodes, app.runtime_stats().fields),
            (1, 0)
        );
        assert!(!app.dispatch(node, Event::Activate).unwrap());
    }
}

#[test]
fn a_panicking_event_restores_the_callers_owner_and_can_be_invoked_again() {
    let app = App::memory();
    let model = app.scope().cage((0usize,));
    let saved = Rc::new(Cell::new(None));
    let out = saved.clone();
    let mut first = true;
    let mounted = button()
        .on(EventKind::Activate, move |_| {
            let field = model.field("value", |v| &v.0, |v| &mut v.0);
            out.set(Some(field));
            if std::mem::take(&mut first) {
                panic!("event failure after path selection");
            }
            field.revise(|value| *value += 1);
        })
        .mount(app.scope())
        .unwrap();
    let node = mounted.roots()[0];
    let caller = app.scope().child().unwrap();
    let caller_alias = caller.owner().enter(|| {
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.dispatch(node, Event::Activate)
        }));
        assert!(failed.is_err());
        model.field("value", |v| &v.0, |v| &mut v.0)
    });
    assert!(app.dispatch(node, Event::Activate).unwrap());
    assert_eq!(caller_alias.get(), 1);
    let listener_alias = saved.get().unwrap();
    drop(mounted);
    app.flush().unwrap();
    assert!(!listener_alias.is_alive());
    assert_eq!(caller_alias.get(), 1);
    caller.dispose();
    assert!(!caller_alias.is_alive());
    assert_eq!(app.runtime_stats().nodes, 1);
}

#[test]
fn an_event_cannot_fall_back_to_the_model_owner_after_disposing_its_listener() {
    let app = App::memory();
    let model = app.scope().cage((0usize,));
    let owner = app.scope().child().unwrap();
    let cancel = owner.clone();
    let result = Rc::new(Cell::new(false));
    let out = result.clone();
    let mounted = button().mount(app.scope()).unwrap();
    let node = mounted.roots()[0];
    owner
        .on(node, EventKind::Activate, move |_| {
            cancel.dispose();
            out.set(matches!(
                model.try_field("value", |v| &v.0, |v| &mut v.0),
                Err(glory_core::AccessError::Disposed)
            ));
        })
        .unwrap();
    assert!(app.dispatch(node, Event::Activate).unwrap());
    assert!(result.get());
    assert!(model.is_alive());
    assert_eq!(
        (app.runtime_stats().nodes, app.runtime_stats().fields),
        (1, 0)
    );
}

struct Host {
    memory: Memory,
    queued: Rc<RefCell<Vec<Input>>>,
}
impl Render for Host {
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
    fn apply(&mut self, commands: &[Mutation]) -> Result<Receipt, Error> {
        self.memory.apply(commands)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        inputs.append(&mut self.queued.borrow_mut());
        self.memory.drain(inputs);
    }
}

#[test]
fn malformed_events_do_not_discard_later_edits_or_query_responses_and_valid_writes_flush() {
    let queued = Rc::new(RefCell::new(Vec::new()));
    let q = queued.clone();
    let app = App::new(move |runtime| Host {
        memory: Memory::new(runtime),
        queued: q,
    });
    let value = app.scope().cage(String::from("initial"));
    app.mount(
        input()
            .value(value)
            .on(EventKind::ValueChanged, move |event| {
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|v| *v = next);
                }
            }),
    )
    .unwrap();
    let node = app.tree().roots()[0];
    let completed = Rc::new(Cell::new(0));
    let seen = completed.clone();
    app.request(Query::Bounds(node), move |result| {
        assert!(matches!(result, Err(Error::Unsupported(_))));
        seen.set(seen.get() + 1);
    })
    .unwrap();
    queued.borrow_mut().extend([
        Input::Event {
            node,
            event: Event::Scroll {
                x: f64::NAN,
                y: 0.,
                viewport: Rect::default(),
                ack: None,
            },
        },
        Input::Event {
            node,
            event: Event::ValueChanged {
                value: "edited🙂".into(),
                composing: false,
                revision: 1,
            },
        },
        Input::Event {
            node,
            event: Event::Commit {
                value: "bad\nline".into(),
                revision: 1,
            },
        },
    ]);
    assert_eq!(
        app.pump(),
        Err(glory_ui::Error::Render(Error::InvalidEvent(
            "invalid scroll geometry"
        )))
    );
    assert_eq!(value.get(), "edited🙂");
    assert_eq!(completed.get(), 1);
    assert_eq!(app.stats().queries, 0);
    assert!(
        matches!(&app.tree().node(node).unwrap().descriptor.view().props, Props::Input(input) if input.value == "edited🙂" && input.revision == 2)
    );
    assert_eq!(app.pump().unwrap(), 0);
    app.clear().unwrap();
}

#[test]
fn a_direct_focus_callback_disposing_its_origin_does_not_publish_stale_focus_within() {
    let app = App::memory();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    app.mount(view().on(EventKind::FocusWithin, move |_| {
        observed.set(observed.get() + 1)
    }))
    .unwrap();
    let root = app.tree().roots()[0];
    let owner = app.scope().child_in(root).unwrap();
    let source = owner
        .create(Descriptor::new(Props::Input(InputProps::default())))
        .unwrap();
    let cancel = owner.clone();
    owner
        .on(source, EventKind::FocusChanged, move |_| cancel.dispose())
        .unwrap();
    app.flush().unwrap();
    assert!(app.dispatch(source, Event::FocusChanged(true)).unwrap());
    assert_eq!(calls.get(), 0);
    assert!(app.tree().node(source).is_err());
    assert!(
        !app.dispatch(
            root,
            Event::FocusWithin {
                node: source,
                focused: false
            }
        )
        .unwrap()
    );
    app.clear().unwrap();
}
