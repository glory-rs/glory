use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory_render::memory::Memory;
use glory_render::*;
use glory_ui::{App, Error as UiError, Widget, input};

#[derive(Default)]
struct Wire {
    applied: u64,
    receipt: Option<u64>,
    inputs: Vec<Input>,
    requests: Vec<(QueryToken, Query, u64)>,
    action_error: Option<Error>,
    actions: Vec<Action>,
}
struct Delayed {
    memory: Memory,
    wire: Rc<RefCell<Wire>>,
}
impl Render for Delayed {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            measure: true,
            scroll: true,
            focus: true,
            input: true,
            collapse_completion: false,
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let receipt = self.memory.apply(mutations)?;
        let mut wire = self.wire.borrow_mut();
        wire.applied = receipt.applied;
        Ok(Receipt {
            applied: wire.receipt.unwrap_or(receipt.applied),
        })
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.node(query.node())?;
        let mut wire = self.wire.borrow_mut();
        let applied = wire.applied;
        wire.requests.push((token, query, applied));
        Ok(())
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        inputs.append(&mut self.wire.borrow_mut().inputs);
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        let mut wire = self.wire.borrow_mut();
        wire.actions.push(action);
        wire.action_error.clone().map_or(Ok(()), Err)
    }
}
fn fixture() -> (App, NodeId, Rc<RefCell<Wire>>) {
    let wire = Rc::new(RefCell::new(Wire::default()));
    let copy = wire.clone();
    let app = App::new(move |id| Delayed {
        memory: Memory::new(id),
        wire: copy,
    });
    app.mount(input().value("a😀z")).unwrap();
    let node = app.tree().roots()[0];
    (app, node, wire)
}

#[test]
fn option_reveal_uses_scroll_capability_and_drops_closed_or_retired_targets() {
    let wire = Rc::new(RefCell::new(Wire::default()));
    let copy = wire.clone();
    let app = App::new(move |id| Delayed {
        memory: Memory::new(id),
        wire: copy,
    });
    let owner = SemanticId::new("reveal-options").unwrap();
    let option = app.scope().cage(None);
    let panel = app.scope().cage(None);
    let metadata = Semantics {
        id: Some(owner.clone()),
        navigation: Some(Orientation::Vertical),
        ..Default::default()
    };
    let mounted = glory_ui::view()
        .role(Role::Listbox)
        .node_ref(panel)
        .semantics(Some(Box::new(metadata.clone())))
        .fill(
            glory_ui::button()
                .label("Option")
                .node_ref(option)
                .focus(FocusMode::Programmatic)
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("reveal-option").unwrap()),
                    option: Some(OptionSemantics { owner }),
                    selected: Some(false),
                    ..Default::default()
                }))),
        )
        .mount(app.scope())
        .unwrap();
    let node = option.get().unwrap();
    assert!(matches!(
        app.scope().action(Action::Focus(node)),
        Err(UiError::Render(Error::Unsupported(_)))
    ));
    assert!(matches!(
        app.scope().action(Action::RevealOption {
            node: panel.get().unwrap()
        }),
        Err(UiError::Render(Error::Unsupported(_)))
    ));
    app.scope().action(Action::RevealOption { node }).unwrap();
    app.flush().unwrap();
    assert_eq!(wire.borrow().actions, [Action::RevealOption { node }]);
    wire.borrow_mut().actions.clear();
    app.scope().action(Action::RevealOption { node }).unwrap();
    let mut hidden = metadata;
    hidden.hidden = true;
    app.scope()
        .patch(
            panel.get().unwrap(),
            Patch::Semantics(Change::Set(Some(Box::new(hidden)))),
        )
        .unwrap();
    app.flush().unwrap();
    assert!(wire.borrow().actions.is_empty());
    app.scope().action(Action::RevealOption { node }).unwrap();
    drop(mounted);
    app.flush().unwrap();
    assert!(wire.borrow().actions.is_empty());
    assert!(matches!(
        app.scope().action(Action::RevealOption { node }),
        Err(UiError::Render(Error::NodeGone(_)))
    ));

    let app = App::memory();
    let owner = SemanticId::new("no-scroll-options").unwrap();
    app.mount(
        glory_ui::view()
            .role(Role::Listbox)
            .semantics(Some(Box::new(Semantics {
                id: Some(owner.clone()),
                navigation: Some(Orientation::Vertical),
                ..Default::default()
            })))
            .fill(
                glory_ui::button()
                    .focus(FocusMode::Programmatic)
                    .semantics(Some(Box::new(Semantics {
                        id: Some(SemanticId::new("no-scroll-option").unwrap()),
                        option: Some(OptionSemantics { owner }),
                        selected: Some(false),
                        ..Default::default()
                    }))),
            ),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let node = app.tree().node(root).unwrap().children[0];
    assert_eq!(
        app.scope().action(Action::RevealOption { node }),
        Err(UiError::Render(Error::Unsupported("scroll")))
    );
}

#[test]
fn menu_exit_survives_the_same_commit_hiding_its_retained_panel() {
    let wire = Rc::new(RefCell::new(Wire::default()));
    let copy = wire.clone();
    let app = App::new(move |id| Delayed {
        memory: Memory::new(id),
        wire: copy,
    });
    let metadata = Semantics {
        id: Some(SemanticId::new("exit-menu").unwrap()),
        navigation: Some(Orientation::Vertical),
        layer: Some(Layer {
            modal: false,
            return_focus: Some(SemanticId::new("menu-trigger").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    };
    app.mount(
        glory_ui::view()
            .role(Role::Menu)
            .focus(FocusMode::Programmatic)
            .semantics(Some(Box::new(metadata.clone()))),
    )
    .unwrap();
    let node = app.tree().roots()[0];
    let mut closed = metadata;
    closed.hidden = true;
    app.scope()
        .patch(node, Patch::Semantics(Change::Set(Some(Box::new(closed)))))
        .unwrap();
    app.scope()
        .action(Action::ExitMenu {
            node,
            backwards: false,
        })
        .unwrap();
    app.scope().action(Action::Focus(node)).unwrap();
    app.flush().unwrap();
    assert!(!app.tree().active(node));
    assert_eq!(
        wire.borrow().actions,
        [Action::ExitMenu {
            node,
            backwards: false
        }]
    );
}
fn reply(wire: &RefCell<Wire>, token: QueryToken, applied: u64, result: Result<QueryValue, Error>) {
    wire.borrow_mut().inputs.push(Input::Response {
        token,
        applied,
        result,
    });
}

#[test]
fn scroll_metrics_obey_owner_retirement_response_kind_and_captured_watermark() {
    let (app, node, wire) = fixture();
    let requester = app.scope().child().unwrap();
    let called = Rc::new(Cell::new(0));
    let out = called.clone();
    let token = requester
        .request(Query::ScrollMetrics(node), move |_| out.set(out.get() + 1))
        .unwrap();
    let accepted = wire.borrow().applied;
    requester.dispose();
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::ScrollMetrics(ScrollMetrics::default())),
    );
    app.pump().unwrap();
    assert_eq!(called.get(), 0);
    for wrong in [
        Ok(QueryValue::Scroll { x: 0.0, y: 0.0 }),
        Ok(QueryValue::ScrollMetrics(ScrollMetrics {
            content_height: f64::NAN,
            ..Default::default()
        })),
    ] {
        let output = Rc::new(RefCell::new(None));
        let saved = output.clone();
        let token = app
            .request(Query::ScrollMetrics(node), move |result| {
                *saved.borrow_mut() = Some(result)
            })
            .unwrap();
        let accepted = wire.borrow().applied;
        reply(&wire, token, accepted, wrong);
        app.pump().unwrap();
        assert!(matches!(
            output.borrow_mut().take().unwrap(),
            Err(Error::InvalidResponse(_))
        ));
    }
    let output = Rc::new(RefCell::new(None));
    let saved = output.clone();
    let token = app
        .request(Query::ScrollMetrics(node), move |result| {
            *saved.borrow_mut() = Some(result)
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    app.scope()
        .patch(node, Patch::Name(Change::Set(Some("later".into()))))
        .unwrap();
    app.flush().unwrap();
    let metrics = ScrollMetrics {
        x: -8.0,
        viewport_width: 100.0,
        content_width: 20.0,
        ..Default::default()
    };
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::ScrollMetrics(metrics)),
    );
    app.pump().unwrap();
    assert_eq!(
        output.borrow_mut().take(),
        Some(Ok(QueryValue::ScrollMetrics(metrics)))
    );
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::ScrollMetrics(metrics)),
    );
    app.pump().unwrap();
    assert!(output.borrow().is_none());
}

#[test]
fn application_query_reply_resources_belong_to_the_app_not_the_pump_caller() {
    let (app, node, wire) = fixture();
    let model = app.scope().cage((7usize,));
    let values = Rc::new(RefCell::new(None));
    let out = values.clone();
    let token = app
        .request(Query::Bounds(node), move |result| {
            result.unwrap();
            let field = model.field("value", |v| &v.0, |v| &mut v.0);
            let derived = model.map(|v| v.0 * 2);
            assert_eq!((field.get(), derived.get()), (7, 14));
            *out.borrow_mut() = Some((field, derived));
        })
        .unwrap();
    let applied = wire.borrow().applied;
    reply(
        &wire,
        token,
        applied,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    let unrelated = app.runtime().owner();
    unrelated.enter(|| app.pump()).unwrap();
    let (field, derived) = values.borrow().unwrap();
    unrelated.dispose();
    assert!(field.is_alive() && derived.is_alive());
    app.clear().unwrap();
    assert!(
        field.is_alive() && derived.is_alive(),
        "App request belongs to the app, not the target"
    );
    drop(app);
    assert!(!field.is_alive() && !derived.is_alive());
}

struct Dropped(Rc<Cell<usize>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn scoped_reply_resources_retire_with_the_requester_including_error_replies() {
    for result in [
        Ok(QueryValue::Bounds(Rect::default())),
        Err(Error::Unsupported("test refusal")),
    ] {
        let (app, node, wire) = fixture();
        let model = app.scope().cage((7usize,));
        let requester = app.scope().child().unwrap();
        let values = Rc::new(RefCell::new(None));
        let out = values.clone();
        let expected = result.clone();
        let token = requester
            .request(Query::Bounds(node), move |result| {
                assert_eq!(result, expected);
                let field = model.field("value", |v| &v.0, |v| &mut v.0);
                let derived = model.map(|v| v.0 * 2);
                assert_eq!((field.get(), derived.get()), (7, 14));
                *out.borrow_mut() = Some((field, derived));
            })
            .unwrap();
        let accepted = wire.borrow().applied;
        reply(&wire, token, accepted, result);
        let caller = app.runtime().owner();
        caller.enter(|| app.pump()).unwrap();
        caller.dispose();
        let (field, derived) = values.borrow().unwrap();
        assert!(field.is_alive() && derived.is_alive());
        requester.dispose();
        assert!(!field.is_alive() && !derived.is_alive());
        assert_eq!(
            (
                app.stats().queries,
                app.stats().query_owners,
                app.stats().query_targets
            ),
            (0, 0, 0)
        );
        assert!(app.tree().node(node).is_ok());
    }
}

#[test]
fn retiring_requesters_release_waiters_promptly_even_for_a_live_external_target() {
    let (app, node, wire) = fixture();
    let drops = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let payload = Rc::new(vec![0_u8; 1024 * 1024]);
    let address = payload.as_ptr();
    for _ in 0..1_000 {
        let requester = app.scope().child().unwrap();
        let mut tokens = Vec::new();
        for _ in 0..2 {
            let captured = Dropped(drops.clone());
            let called = calls.clone();
            let data = payload.clone();
            tokens.push(
                requester
                    .request(Query::Bounds(node), move |_| {
                        let _hold = captured;
                        assert_eq!(data.as_ptr(), address);
                        called.set(called.get() + 1);
                    })
                    .unwrap(),
            );
        }
        assert_eq!(
            (
                app.stats().queries,
                app.stats().query_owners,
                app.stats().query_targets
            ),
            (2, 1, 1)
        );
        let before = drops.get();
        requester.dispose();
        assert_eq!(
            drops.get(),
            before + 2,
            "release before another flush or renderer response"
        );
        assert_eq!(
            (
                app.stats().queries,
                app.stats().query_owners,
                app.stats().query_targets
            ),
            (0, 0, 0)
        );
        let accepted = wire.borrow().applied;
        for token in tokens {
            reply(
                &wire,
                token,
                accepted,
                Ok(QueryValue::Bounds(Rect::default())),
            );
        }
        app.pump().unwrap();
        wire.borrow_mut().requests.clear();
        assert!(app.runtime_stats().owner_slots < 10);
    }
    assert_eq!(
        (calls.get(), drops.get(), Rc::strong_count(&payload)),
        (0, 2_000, 1)
    );
    assert!(app.tree().node(node).is_ok());
}

#[test]
fn requester_retirement_also_drops_already_cancelled_target_replies() {
    let (app, _, wire) = fixture();
    let target = app.scope().child().unwrap();
    let node = target
        .create(Descriptor::new(Props::Input(Default::default())))
        .unwrap();
    let requester = app.scope().child().unwrap();
    let drops = Rc::new(Cell::new(0));
    let captured = Dropped(drops.clone());
    let token = requester
        .request(Query::Bounds(node), move |_| {
            let _hold = captured;
            panic!("requester retired before cancellation delivery");
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    target.dispose();
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (1, 1, 0)
    );
    requester.dispose();
    assert_eq!(drops.get(), 1);
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(drops.get(), 1);
}

#[test]
fn target_cancellation_wins_a_response_in_the_same_drained_batch() {
    let (app, _, wire) = fixture();
    let target = app.scope().child().unwrap();
    let node = target
        .create(Descriptor::new(Props::Input(Default::default())))
        .unwrap();
    let button = app
        .scope()
        .create(Descriptor::new(Props::Button(Default::default())))
        .unwrap();
    let retired = target.clone();
    app.scope()
        .on(button, EventKind::Activate, move |_| retired.dispose())
        .unwrap();
    let requester = app.scope().child().unwrap();
    let model = app.scope().cage((3usize,));
    let values = Rc::new(RefCell::new(None));
    let out = values.clone();
    let token = requester
        .request(Query::Bounds(node), move |result| {
            assert_eq!(result, Err(Error::NodeGone(node)));
            *out.borrow_mut() = Some(model.field("value", |v| &v.0, |v| &mut v.0));
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    wire.borrow_mut().inputs.push(Input::Event {
        node: button,
        event: Event::Activate,
    });
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    assert_eq!(app.pump().unwrap(), 1);
    let field = values.borrow().unwrap();
    assert!(field.is_alive());
    requester.dispose();
    assert!(!field.is_alive());
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
    app.pump().unwrap();
}

#[test]
fn a_query_barrier_that_retires_the_requester_does_not_send_a_request() {
    let (app, node, wire) = fixture();
    let requester = app.scope().child().unwrap();
    let trigger = app.scope().cage(false);
    let retiring = requester.clone();
    app.scope().effect(move || {
        if trigger.get() {
            retiring.dispose();
        }
    });
    app.flush().unwrap();
    trigger.revise(|v| *v = true);
    let drops = Rc::new(Cell::new(0));
    let captured = Dropped(drops.clone());
    assert!(matches!(
        requester.request(Query::Bounds(node), move |_| {
            let _hold = captured;
            panic!("request not accepted");
        }),
        Err(UiError::Access(glory_core::AccessError::Disposed))
    ));
    assert_eq!(drops.get(), 1);
    assert!(wire.borrow().requests.is_empty());
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
}

#[test]
fn a_panicking_reply_restores_the_callers_owner_and_is_not_delivered_again() {
    let (app, node, wire) = fixture();
    let requester = app.scope().child().unwrap();
    let model = app.scope().cage((3usize,));
    let values = Rc::new(RefCell::new(None));
    let out = values.clone();
    let token = requester
        .request(Query::Bounds(node), move |_| {
            *out.borrow_mut() = Some(model.field("value", |v| &v.0, |v| &mut v.0));
            panic!("reply unwind");
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    let caller = app.runtime().owner();
    let other = caller.enter(|| {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.pump())).is_err());
        model.field("value", |v| &v.0, |v| &mut v.0)
    });
    let field = values.borrow().unwrap();
    caller.dispose();
    assert!(!other.is_alive() && field.is_alive());
    requester.dispose();
    assert!(!field.is_alive());
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
}

#[test]
fn responses_must_match_the_request_barrier_even_after_later_commits() {
    let (app, node, wire) = fixture();
    let replies = Rc::new(RefCell::new(Vec::new()));
    for delta in [-1, 1, 0] {
        let received = replies.clone();
        let token = app
            .request(Query::Selection(node), move |value| {
                received.borrow_mut().push(value)
            })
            .unwrap();
        let accepted = wire.borrow().applied;
        let watermark = if delta < 0 {
            accepted - 1
        } else {
            accepted + delta as u64
        };
        reply(
            &wire,
            token,
            watermark,
            Ok(QueryValue::Selection { start: 1, end: 3 }),
        );
        // This is not a reason to reject a correctly captured older response.
        app.scope()
            .patch(
                node,
                Patch::Value {
                    value: Change::Set("later".into()),
                    revision: accepted,
                },
            )
            .unwrap();
        app.flush().unwrap();
        app.pump().unwrap();
        let expected = if delta == 0 {
            Ok(QueryValue::Selection { start: 1, end: 3 })
        } else {
            Err(Error::QueryWatermark {
                expected: accepted,
                applied: watermark,
            })
        };
        assert_eq!(replies.borrow().last(), Some(&expected));
        assert_eq!(app.stats().queries, 0);
        // Duplicate/unknown replies do not call application code again.
        reply(
            &wire,
            token,
            watermark,
            Ok(QueryValue::Selection { start: 0, end: 0 }),
        );
        let count = replies.borrow().len();
        app.pump().unwrap();
        assert_eq!(replies.borrow().len(), count);
    }
    assert_eq!(replies.borrow().len(), 3);
}

#[test]
fn invalid_query_payloads_complete_once_and_do_not_poison_the_session() {
    let (app, node, wire) = fixture();
    let cases = [
        (
            Query::Bounds(node),
            QueryValue::Scroll { x: 0.0, y: 0.0 },
            Some("query result kind mismatch"),
        ),
        (
            Query::Bounds(node),
            QueryValue::Bounds(Rect {
                width: -1.0,
                ..Rect::default()
            }),
            Some("invalid bounds"),
        ),
        (
            Query::Bounds(node),
            QueryValue::Bounds(Rect {
                x: f64::NAN,
                ..Rect::default()
            }),
            Some("invalid bounds"),
        ),
        (
            Query::Scroll(node),
            QueryValue::Scroll {
                x: 0.0,
                y: f64::INFINITY,
            },
            Some("invalid scroll offsets"),
        ),
        (
            Query::Selection(node),
            QueryValue::Selection { start: 3, end: 1 },
            Some("reversed selection"),
        ),
        (
            Query::Scroll(node),
            QueryValue::Scroll { x: -3.5, y: -1.0 },
            None,
        ),
        (
            Query::Selection(node),
            QueryValue::Selection { start: 1, end: 3 },
            None,
        ),
        (Query::Focus(node), QueryValue::Focus(true), None),
        (Query::Focus(node), QueryValue::Focus(false), None),
        (
            Query::Focus(node),
            QueryValue::Selection { start: 0, end: 0 },
            Some("query result kind mismatch"),
        ),
    ];
    let replies = Rc::new(RefCell::new(Vec::new()));
    let finished = app.scope().cage(0);
    for (query, value, message) in cases {
        let received = replies.clone();
        let token = app
            .request(query, move |value| {
                finished.revise(|count| *count += 1);
                received.borrow_mut().push(value);
            })
            .unwrap();
        let applied = wire.borrow().applied;
        reply(&wire, token, applied, Ok(value.clone()));
        app.pump().unwrap();
        assert_eq!(
            replies.borrow().last(),
            Some(&message.map_or(Ok(value), |message| Err(Error::InvalidResponse(message))))
        );
        assert_eq!(app.stats().queries, 0);
    }
    assert_eq!(finished.get(), 10);
    app.scope()
        .patch(
            node,
            Patch::Value {
                value: Change::Set("recovered".into()),
                revision: 2,
            },
        )
        .unwrap();
    app.flush().unwrap();
}

#[test]
fn regressed_or_unadvanced_commit_receipts_fault_the_app() {
    for unchanged in [false, true] {
        let (app, node, wire) = fixture();
        let previous = wire.borrow().applied;
        let applied = previous - u64::from(!unchanged);
        wire.borrow_mut().receipt = Some(applied);
        app.scope()
            .patch(
                node,
                Patch::Value {
                    value: Change::Set("next".into()),
                    revision: 2,
                },
            )
            .unwrap();
        assert_eq!(
            app.flush(),
            Err(UiError::Render(Error::InvalidReceipt { previous, applied }))
        );
        assert_eq!(app.flush(), Err(UiError::Faulted));
        assert_eq!(
            app.scope().patch(
                node,
                Patch::Value {
                    value: Change::Clear,
                    revision: 2
                }
            ),
            Err(UiError::Faulted)
        );
    }
}

#[test]
fn disposed_query_waiters_ignore_late_responses_for_reused_slots() {
    let (app, node, wire) = fixture();
    let replies = Rc::new(RefCell::new(Vec::new()));
    let received = replies.clone();
    let token = app
        .request(Query::Bounds(node), move |value| {
            received.borrow_mut().push(value)
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    app.clear().unwrap();
    app.mount(input()).unwrap();
    let replacement = app.tree().roots()[0];
    assert_eq!(node.slot(), replacement.slot());
    assert_ne!(node, replacement);
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(&*replies.borrow(), &[Err(Error::NodeGone(node))]);
    assert_eq!(app.stats().queries, 0);
}

#[test]
fn ordinary_focus_refusal_continues_but_backend_failures_still_fault() {
    for (error, focus, recoverable) in [
        (Error::Unsupported("focus refused"), true, true),
        (Error::Backend("lost host".into()), true, false),
        (Error::Unsupported("scroll unavailable"), false, false),
    ] {
        let (app, node, wire) = fixture();
        wire.borrow_mut().action_error = Some(error.clone());
        let action = if focus {
            Action::Focus(node)
        } else {
            Action::ScrollTo { node, x: 0., y: 0. }
        };
        app.scope().action(action).unwrap();
        app.scope().action(action).unwrap();
        assert_eq!(
            app.flush(),
            if recoverable {
                Ok(())
            } else {
                Err(UiError::Render(error))
            }
        );
        assert_eq!(wire.borrow().actions.len(), if recoverable { 2 } else { 1 });
        wire.borrow_mut().action_error = None;
        if recoverable {
            app.flush().unwrap();
            app.scope().action(Action::Focus(node)).unwrap();
            app.flush().unwrap();
            assert_eq!(wire.borrow().actions.len(), 3);
        } else {
            assert_eq!(app.flush(), Err(UiError::Faulted));
        }
    }
}

#[test]
fn owner_interests_survive_independent_node_listener_and_query_retirement() {
    let (app, external, wire) = fixture();
    let baseline = app.stats();
    let owner = app.scope().child().unwrap();
    let node = owner
        .create(Descriptor::new(Props::Button(Default::default())))
        .unwrap();
    let listener = app.scope().child().unwrap();
    listener.on(node, EventKind::Activate, |_| {}).unwrap();
    let calls = Rc::new(Cell::new(0));
    let out = calls.clone();
    let pending = listener
        .request(Query::Bounds(external), move |result| {
            result.unwrap();
            out.set(out.get() + 1);
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    owner.dispose(); // Removes listener's sole target, but not its pending query.
    app.flush().unwrap();
    assert_eq!(app.stats().owners, baseline.owners);
    assert_eq!(app.stats().query_owners, 1);
    reply(
        &wire,
        pending,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(app.stats().query_owners, 0);
    // The same live Owner can acquire new interests after its last one retires.
    listener
        .on(external, EventKind::FocusChanged, |_| {})
        .unwrap();
    let drops = Rc::new(Cell::new(0));
    let capture = Dropped(drops.clone());
    let cancelled = listener
        .request(Query::Bounds(external), move |_| {
            std::hint::black_box(&capture);
            panic!("retired requester");
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    listener.dispose();
    app.flush().unwrap();
    assert_eq!(drops.get(), 1);
    reply(
        &wire,
        cancelled,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(calls.get(), 1);
    let stats = app.stats();
    assert_eq!(
        (
            stats.nodes,
            stats.owners,
            stats.handlers,
            stats.queries,
            stats.query_owners,
            stats.query_targets
        ),
        (
            baseline.nodes,
            baseline.owners,
            baseline.handlers,
            baseline.queries,
            baseline.query_owners,
            baseline.query_targets
        )
    );
}

#[test]
fn explicit_query_cancellation_checks_requester_and_drops_reentrant_captures_outside_registry() {
    struct Probe {
        writes: glory_core::Cage<usize>,
        scope: glory_ui::WeakScope,
        node: NodeId,
        next: Rc<Cell<Option<QueryToken>>>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            self.writes.revise(|value| *value += 1);
            let scope = self.scope.upgrade().unwrap();
            self.next.set(Some(
                scope.request(Query::Bounds(self.node), |_| {}).unwrap(),
            ));
        }
    }
    let (app, node, wire) = fixture();
    let requester = app.scope().child().unwrap();
    let sibling = app.scope().child().unwrap();
    let writes = app.scope().cage(0);
    let next = Rc::new(Cell::new(None));
    let captured = Probe {
        writes,
        scope: requester.downgrade(),
        node,
        next: next.clone(),
    };
    let token = requester
        .request(Query::Bounds(node), move |_| {
            let _hold = captured;
            panic!("cancelled reply");
        })
        .unwrap();
    let accepted = wire.borrow().applied;
    assert!(sibling.cancel_request(token).is_err());
    assert!(app.cancel_request(token).is_err());
    assert_eq!(app.stats().queries, 1);
    assert_eq!(writes.get(), 0);
    assert!(requester.cancel_request(token).unwrap());
    assert_eq!(writes.get(), 1);
    let reentrant = next.get().unwrap();
    assert_eq!(app.stats().queries, 1);
    assert!(!requester.cancel_request(token).unwrap());
    assert!(!requester.cancel_request(QueryToken(u64::MAX)).unwrap());
    assert!(requester.cancel_request(reentrant).unwrap());
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    reply(
        &wire,
        reentrant,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    requester.dispose();
    assert!(requester.cancel_request(token).is_err());
    let root = app
        .request(Query::Bounds(node), |_| panic!("cancelled app query"))
        .unwrap();
    assert!(app.cancel_request(root).unwrap());
    assert!(!app.cancel_request(root).unwrap());
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
}

#[test]
fn explicit_cancel_also_revokes_queued_target_gone_reply_without_retaining_owner_buckets() {
    let (app, _, wire) = fixture();
    let target = app.scope().child().unwrap();
    let node = target
        .create(Descriptor::new(Props::Input(Default::default())))
        .unwrap();
    let requester = app.scope().child().unwrap();
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    let token = requester
        .request(Query::Bounds(node), move |_| seen.set(seen.get() + 1))
        .unwrap();
    let accepted = wire.borrow().applied;
    target.dispose();
    assert_eq!(app.stats().queries, 1);
    assert_eq!(app.stats().query_targets, 0);
    assert!(requester.cancel_request(token).unwrap());
    app.pump().unwrap();
    reply(
        &wire,
        token,
        accepted,
        Ok(QueryValue::Bounds(Rect::default())),
    );
    app.pump().unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(
        (
            app.stats().queries,
            app.stats().query_owners,
            app.stats().query_targets
        ),
        (0, 0, 0)
    );
}
