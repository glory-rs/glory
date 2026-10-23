use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory_render::memory::Memory;
use glory_render::{
    Action, Capabilities, Error, Event, Input as RenderInput, Mutation, NodeId, Props, Query,
    QueryToken, Receipt, Rect, Render, Role,
};
use glory_ui::{App, Widget, button, component, list, text};

#[test]
fn event_created_paths_and_derived_values_retire_with_huge_list_rows_except_the_live_focus_pin() {
    let (app, observed) = app();
    let model = app.scope().cage((0usize, vec![9u8; 1024 * 1024]));
    let payload = model.with(|value| value.1.as_ptr());
    let position = app.scope().cage(0.);
    let latest = Rc::new(RefCell::new(None));
    let inspect = latest.clone();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    let last = Rc::new(Cell::new(None));
    let out = last.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            move |_, key| {
                let out = out.clone();
                let mut derived = None;
                button().fill(text().content(format!("event:{key}"))).on(
                    glory_render::EventKind::Activate,
                    move |_| {
                        let field = model.field("count", |value| &value.0, |value| &mut value.0);
                        let derived = *derived.get_or_insert_with(|| field.map(|value| *value));
                        field.revise(|value| *value += 1);
                        assert_eq!(field.get(), derived.get());
                        out.set(Some((field, derived)));
                    },
                )
            },
        )
        .height(240.)
        .overscan(2)
        .row_limit(20)
        .position(position)
        .inspect(move |value| *inspect.borrow_mut() = Some(value)),
    )
    .unwrap();
    let focused = app.tree().node(rows(&app)[0]).unwrap().children[0];
    app.dispatch(focused, Event::Activate).unwrap();
    let pin = last.take().unwrap();
    app.dispatch(focused, Event::FocusChanged(true)).unwrap();
    let mut previous = None;
    let (mut peak_nodes, mut peak_slots, mut peak_owners) = (0, 0, 0);
    for step in 1..=300 {
        position.revise(|value| *value = ((step * 7919) % 100_000) as f64 * 24_000.);
        app.flush().unwrap();
        assert!(pin.0.is_alive() && pin.1.is_alive());
        if let Some((field, derived)) = previous.take() {
            assert!(!glory_core::Cage::is_alive(field) && !glory_core::Bond::is_alive(derived));
        }
        let target = rows(&app)
            .into_iter()
            .map(|row| app.tree().node(row).unwrap().children[0])
            .find(|node| *node != focused)
            .unwrap();
        let before = app.runtime_stats().nodes;
        let key_reads = reads.get();
        for _ in 0..5 {
            app.dispatch(target, Event::Activate).unwrap();
            assert_eq!(app.runtime_stats().nodes, before + 2);
        }
        assert_eq!(reads.get(), key_reads);
        assert_eq!(latest.borrow().as_ref().unwrap().pinned, 1);
        assert!(latest.borrow().as_ref().unwrap().rows <= 21);
        assert_eq!(app.runtime_stats().fields, 1);
        assert!(app.runtime_stats().node_slots <= 80 && app.runtime_stats().owner_slots <= 120);
        let stats = app.runtime_stats();
        peak_nodes = peak_nodes.max(stats.nodes);
        peak_slots = peak_slots.max(stats.node_slots);
        peak_owners = peak_owners.max(stats.owner_slots);
        assert_eq!(model.with(|value| value.1.as_ptr()), payload);
        previous = last.take();
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    app.dispatch(focused, Event::FocusChanged(false)).unwrap();
    assert!(!pin.0.is_alive() && !pin.1.is_alive());
    app.clear().unwrap();
    let (field, derived) = previous.unwrap();
    assert!(!field.is_alive() && !derived.is_alive());
    assert_eq!(
        (app.runtime_stats().fields, app.runtime_stats().nodes),
        (0, 2)
    );
    assert_eq!(model.with(|value| value.0), 1501);
    assert!(reads.get() < 7000);
    println!(
        "event-owned huge list: key_reads={}, peak_nodes={peak_nodes}, peak_node_slots={peak_slots}, peak_owner_slots={peak_owners}",
        reads.get()
    );
}

#[test]
fn a_far_jump_retires_rows_before_a_large_unrelated_binding_backlog_without_scanning_it() {
    let app = App::memory();
    let position = app.scope().cage(0.);
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            |_, _| text().content("row"),
        )
        .height(240.)
        .overscan(2)
        .position(position),
    )
    .unwrap();
    let previous = rows(&app);
    let calls = Rc::new(Cell::new(0));
    for index in 0..16_384 {
        let called = calls.clone();
        app.scope().binding(move || {
            assert_eq!(
                called.get(),
                index,
                "live binding FIFO must survive structural row retirement"
            );
            called.set(called.get() + 1);
        });
    }
    assert_eq!(app.runtime_stats().queued, 16_384);
    let before = app.runtime_stats().queue_compaction_visits;
    let initial_reads = reads.get();
    position.revise(|value| *value = 1_234_567_890.);
    app.flush().unwrap();
    assert_eq!(calls.get(), 16_384);
    assert!(previous.iter().all(|node| app.tree().node(*node).is_err()));
    assert!(reads.get() - initial_reads < 32);
    assert!(rows(&app).len() < 20);
    let stats = app.runtime_stats();
    assert_eq!((stats.queued, stats.queue_entries), (0, 0));
    assert_eq!(
        stats.queue_compaction_visits, before,
        "clean row nodes must not scan unrelated queues"
    );
    app.clear().unwrap();
    println!(
        "queue-backed huge list: background_bindings={}, new_key_reads={}, compaction_visits={}",
        calls.get(),
        reads.get() - initial_reads,
        stats.queue_compaction_visits - before
    );
}

#[derive(Default)]
struct Observed {
    mutations: Vec<Mutation>,
    actions: Vec<Action>,
    inputs: Vec<RenderInput>,
    commits: usize,
    no_focus: bool,
}

#[test]
fn huge_list_row_queries_release_captures_before_late_replies_without_retaining_visited_rows() {
    struct Captured(Rc<Cell<usize>>, Rc<Vec<u8>>);
    impl Drop for Captured {
        fn drop(&mut self) {
            assert_eq!(self.1.len(), 1024 * 1024);
            self.0.set(self.0.get() + 1);
        }
    }
    let (app, observed) = app();
    let external = app
        .scope()
        .create(glory_render::Descriptor::new(Props::Input(
            Default::default(),
        )))
        .unwrap();
    let position = app.scope().cage(0.);
    let payload = Rc::new(vec![0_u8; 1024 * 1024]);
    let drops = Rc::new(Cell::new(0));
    let data = payload.clone();
    let retired = drops.clone();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            move |_, _| {
                let data = data.clone();
                let retired = retired.clone();
                component(move |scope| {
                    let requester = scope.clone();
                    let data = data.clone();
                    let retired = retired.clone();
                    button()
                        .on(glory_render::EventKind::Activate, move |_| {
                            let captured = Captured(retired.clone(), data.clone());
                            requester
                                .request(Query::Bounds(external), move |_| {
                                    let _hold = captured;
                                    panic!(
                                        "departed row must not receive a late external-node reply"
                                    );
                                })
                                .unwrap();
                        })
                        .show_in(scope)
                })
            },
        )
        .height(240.)
        .overscan(2)
        .row_limit(20)
        .position(position),
    )
    .unwrap();
    let list_root = app.tree().roots()[1];
    let live_rows = || {
        let tree = app.tree();
        let body = tree.node(list_root).unwrap().children[0];
        tree.node(body).unwrap().children.clone()
    };
    let mut peak_owners = 0;
    for step in 1..=300 {
        let target = app.tree().node(live_rows()[0]).unwrap().children[0];
        let before_reads = reads.get();
        for _ in 0..5 {
            app.dispatch(target, Event::Activate).unwrap();
        }
        assert_eq!(reads.get(), before_reads);
        assert_eq!(
            (
                app.stats().queries,
                app.stats().query_owners,
                app.stats().query_targets
            ),
            (5, 1, 1)
        );
        position.revise(|value| *value = ((step * 7919) % 100_000) as f64 * 24_000.);
        app.flush().unwrap();
        assert_eq!(drops.get(), step * 5);
        assert_eq!(
            (
                app.stats().queries,
                app.stats().query_owners,
                app.stats().query_targets
            ),
            (0, 0, 0)
        );
        // Memory queued real responses at acceptance. Draining them after row
        // retirement must neither call the released closures nor grow history.
        app.pump().unwrap();
        assert!(app.tree().node(external).is_ok());
        assert!(live_rows().len() <= 20 && app.runtime_stats().owner_slots <= 110);
        peak_owners = peak_owners.max(app.runtime_stats().owner_slots);
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    app.clear().unwrap();
    assert_eq!(drops.get(), 1_500);
    assert_eq!(Rc::strong_count(&payload), 1);
    assert!(reads.get() < 7_000);
    println!(
        "query-owned huge list: key_reads={}, replies_dropped={}, peak_owner_slots={peak_owners}",
        reads.get(),
        drops.get()
    );
}
struct Host {
    memory: Memory,
    observed: Rc<RefCell<Observed>>,
}
impl Render for Host {
    fn style_profile(&self) -> glory_render::style::Profile {
        self.memory.style_profile()
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            scroll: true,
            focus: !self.observed.borrow().no_focus,
            ..Capabilities::default()
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let mut observed = self.observed.borrow_mut();
        observed.mutations.extend_from_slice(mutations);
        observed.commits += 1;
        self.memory.apply(mutations)
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        self.memory.node(action.node())?;
        self.observed.borrow_mut().actions.push(action);
        Ok(())
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.memory.request(query, token)
    }
    fn drain(&mut self, inputs: &mut Vec<RenderInput>) {
        inputs.append(&mut self.observed.borrow_mut().inputs);
        self.memory.drain(inputs);
    }
}
fn app() -> (App, Rc<RefCell<Observed>>) {
    let observed = Rc::new(RefCell::new(Observed::default()));
    let copy = observed.clone();
    (
        App::new(move |id| Host {
            memory: Memory::new(id),
            observed: copy,
        }),
        observed,
    )
}
fn scroll_event(y: f64) -> Event {
    Event::Scroll {
        ack: None,
        x: 0.0,
        y,
        viewport: Rect {
            width: 800.0,
            height: 600.0,
            ..Rect::default()
        },
    }
}

#[test]
fn field_aliases_on_a_hundred_million_rows_follow_only_live_row_owners() {
    struct Model {
        prefix: String,
        background: usize,
        payload: Vec<u8>,
    }
    let (app, observed) = app();
    let model = app.scope().cage(Model {
        prefix: "a".into(),
        background: 0,
        payload: vec![9; 1024 * 1024],
    });
    let pointer = model.with(|m| m.payload.as_ptr());
    let writers = app.runtime().owner();
    let (prefix, background) = writers.enter(|| {
        (
            model.field("prefix", |m| &m.prefix, |m| &mut m.prefix),
            model.field("background", |m| &m.background, |m| &mut m.background),
        )
    });
    let position = app.scope().cage(0.0);
    let keys = Rc::new(Cell::new(0usize));
    let reads = keys.clone();
    let computations = Rc::new(Cell::new(0usize));
    let updates = computations.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                reads.set(reads.get() + 1);
                index
            },
            move |_, key| {
                let updates = updates.clone();
                component(move |scope| {
                    let prefix = model.field("prefix", |m| &m.prefix, |m| &mut m.prefix);
                    let updates = updates.clone();
                    text()
                        .content(prefix.map(move |prefix| {
                            updates.set(updates.get() + 1);
                            format!("{prefix}:{key}")
                        }))
                        .show_in(scope)
                })
            },
        )
        .height(180.)
        .overscan(2)
        .row_limit(20)
        .position(position),
    )
    .unwrap();
    for jump in 0..300 {
        position.revise(|y| *y = ((jump * 104729) % 100_000_000) as f64 * 24.);
        app.flush().unwrap();
        let live = labels(&app).len();
        assert!((1..=20).contains(&live));
        assert_eq!(app.runtime_stats().fields, 2);
        assert!(app.runtime_stats().field_slots <= 2);
        assert!(app.runtime_stats().node_slots <= 180);
        assert!(app.runtime_stats().owner_slots <= 180);
        let before = app.runtime_stats();
        let read_count = keys.get();
        let compute_count = computations.get();
        observed.borrow_mut().mutations.clear();
        background.revise(|n| *n += 1);
        app.flush().unwrap();
        assert_eq!(keys.get(), read_count);
        assert_eq!(computations.get(), compute_count);
        assert_eq!(app.runtime_stats().derived_runs, before.derived_runs);
        assert!(observed.borrow().mutations.is_empty());
        let next = format!("p{jump}");
        prefix.revise(|v| *v = next.clone());
        app.flush().unwrap();
        assert_eq!(keys.get(), read_count);
        assert_eq!(computations.get() - compute_count, live);
        assert!(
            labels(&app)
                .iter()
                .all(|(_, label)| label.starts_with(&format!("{next}:")))
        );
        assert_eq!(model.with(|m| m.payload.as_ptr()), pointer);
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    // Whole-model writes retain conservative notifications for every live field.
    model.revise(|m| m.prefix = "root".into());
    app.flush().unwrap();
    assert!(
        labels(&app)
            .iter()
            .all(|(_, label)| label.starts_with("root:"))
    );
    drop(writers);
    assert_eq!(app.runtime_stats().fields, 1);
    app.clear().unwrap();
    assert_eq!(app.runtime_stats().fields, 0);
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.stats().handlers, 0);
    assert!(model.is_alive());
    assert!(keys.get() < 7000); // Indexed, never a scan of the source or visited rows.
}

#[test]
fn keyed_collection_paths_on_sparse_huge_lists_reclaim_each_visited_key_and_update_one_row() {
    struct Row {
        name: String,
        payload: Vec<u8>,
    }
    let (app, observed) = app();
    // A sparse business cache, independent of the 100M logical list. Framework
    // paths must not retain either visited keys or removed business values.
    let source = app
        .scope()
        .cage(std::collections::HashMap::<usize, Row>::new());
    let position = app.scope().cage(0.);
    let keys = Rc::new(Cell::new(0usize));
    let reads = keys.clone();
    let computations = Rc::new(Cell::new(0usize));
    let runs = computations.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                reads.set(reads.get() + 1);
                index
            },
            move |_, key| {
                let runs = runs.clone();
                component(move |scope| {
                    let row = source.entry(key);
                    let runs = runs.clone();
                    let label = scope
                        .bond(move || {
                            runs.set(runs.get() + 1);
                            format!(
                                "{key}:{}",
                                row.try_with(|r| r.name.clone())
                                    .unwrap_or_else(|_| "missing".into())
                            )
                        })
                        .with_partial_eq();
                    text().content(label).show_in(scope)
                })
            },
        )
        .height(180.)
        .overscan(2)
        .row_limit(20)
        .position(position),
    )
    .unwrap();
    let mut loaded = None;
    for jump in 0..300 {
        position.revise(|y| *y = ((jump * 104729) % 100_000_000) as f64 * 24.);
        app.flush().unwrap();
        let row_labels = labels(&app);
        let live = row_labels.len();
        assert!((1..=20).contains(&live));
        let key: usize = row_labels[0].1.split(':').next().unwrap().parse().unwrap();
        let before_load = computations.get();
        let reads_before_load = keys.get();
        if let Some(old) = loaded.take() {
            source.remove(&old);
        }
        source.insert(
            key,
            Row {
                name: "loaded".into(),
                payload: vec![9; 1024 * 1024],
            },
        );
        loaded = Some(key);
        app.flush().unwrap();
        assert_eq!(
            computations.get() - before_load,
            1,
            "loading one key must not recompute other visible rows"
        );
        assert_eq!(
            keys.get(),
            reads_before_load,
            "data publication must not re-enumerate the window"
        );
        let writer = app.runtime().owner();
        let item = writer.enter(|| source.entry(key));
        let name = writer.enter(|| item.field("name", |r| &r.name, |r| &mut r.name));
        let pointer = item.with(|r| r.payload.as_ptr());
        assert_eq!(app.runtime_stats().entry_keys, live);
        assert_eq!(app.runtime_stats().entry_buckets, live);
        assert_eq!(app.runtime_stats().fields, live + 1);
        let before = computations.get();
        let read_count = keys.get();
        observed.borrow_mut().mutations.clear();
        name.revise(|s| s.push('!'));
        app.flush().unwrap();
        assert_eq!(computations.get() - before, 1);
        assert_eq!(keys.get(), read_count);
        assert_eq!(item.with(|r| r.payload.as_ptr()), pointer);
        assert_eq!(
            labels(&app)
                .iter()
                .filter(|(_, s)| s.ends_with("loaded!"))
                .count(),
            1
        );
        drop(writer);
        assert_eq!(app.runtime_stats().fields, live);
        assert!(
            app.runtime_stats().field_slots <= 21
                && app.runtime_stats().node_slots <= 180
                && app.runtime_stats().owner_slots <= 180
        );
        assert_eq!(source.with(|m| m.len()), 1);
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    app.clear().unwrap();
    assert_eq!(
        (
            app.runtime_stats().fields,
            app.runtime_stats().entry_keys,
            app.runtime_stats().entry_buckets
        ),
        (0, 0, 0)
    );
    assert_eq!(app.stats().nodes, 0);
    assert!(keys.get() < 7000);
}

#[test]
fn invalid_geometry_and_stale_origins_do_not_touch_a_hundred_million_row_source() {
    let (app, observed) = app();
    let position = app.scope().cage(0.);
    let reads = Rc::new(Cell::new(0usize));
    let keys = reads.clone();
    let latest = Rc::new(RefCell::new(None));
    let stats = latest.clone();
    app.mount(
        list(
            100_000_000usize,
            move |i| {
                keys.set(keys.get() + 1);
                i
            },
            |_, key| text().content(format!("row:{key}")),
        )
        .height(180.)
        .overscan(2)
        .row_limit(20)
        .measure(24.)
        .measure_limit(64)
        .position(position)
        .inspect(move |value| *stats.borrow_mut() = Some(value)),
    )
    .unwrap();
    let receiver = root(&app);
    let old_row = rows(&app)[0];
    let outside = glory_ui::view().mount(app.scope()).unwrap();
    app.flush().unwrap();
    let viewport = Rect {
        width: 200.,
        height: 180.,
        ..Rect::default()
    };
    let before = reads.get();
    let nodes = app.stats().nodes;
    let actions = observed.borrow().actions.len();
    for event in [
        Event::Scroll {
            x: 0.,
            y: f64::NAN,
            viewport,
            ack: None,
        },
        Event::Scroll {
            x: 0.,
            y: 0.,
            viewport,
            ack: Some(glory_render::ScrollAck {
                requested: glory_render::ScrollRequest {
                    x: 0.,
                    y: f64::INFINITY,
                },
                x: 0.,
                y: 0.,
            }),
        },
        Event::Resize {
            size: Rect {
                height: -1.,
                ..Rect::default()
            },
            viewport,
        },
        Event::Resize {
            size: Rect::default(),
            viewport: Rect { x: 1., ..viewport },
        },
        Event::Anchor {
            row: old_row,
            within: f64::INFINITY,
            viewport,
        },
        Event::Anchor {
            row: outside.roots()[0],
            within: 7.,
            viewport,
        },
        Event::FocusWithin {
            node: outside.roots()[0],
            focused: true,
        },
    ] {
        assert!(matches!(
            app.dispatch(receiver, event),
            Err(glory_ui::Error::Render(Error::InvalidEvent(_)))
        ));
        assert_eq!(reads.get(), before);
        assert_eq!(app.stats().nodes, nodes);
        assert_eq!(observed.borrow().actions.len(), actions);
        assert_eq!(latest.borrow().as_ref().unwrap().pinned, 0);
    }
    app.dispatch(old_row, measure_event(32.)).unwrap();
    position.revise(|value| *value = 24_000_007.);
    app.flush().unwrap();
    assert!(app.tree().node(old_row).is_err());
    let reads_after_seek = reads.get();
    assert!(
        !app.dispatch(
            receiver,
            Event::Anchor {
                row: old_row,
                within: 7.,
                viewport
            }
        )
        .unwrap()
    );
    assert!(
        !app.dispatch(
            receiver,
            Event::FocusWithin {
                node: old_row,
                focused: true
            }
        )
        .unwrap()
    );
    assert_eq!(reads.get(), reads_after_seek);
    for step in 1..=30 {
        position.revise(|value| *value = step as f64 * 71_923_000. + 7.);
        app.flush().unwrap();
        let s = latest.borrow();
        let s = s.as_ref().unwrap();
        // 180px can intersect nine 24px rows at a fractional inset, plus four
        // overscan rows. This viewport is larger than the 120px raster fixture.
        assert!(
            s.rows <= 13 && s.measured <= 64 && s.height_capacity < 4096 && s.pinned == 0,
            "{s:?}"
        );
        assert!(app.stats().nodes < 64 && app.runtime_stats().node_slots < 256);
    }
    assert!(reads.get() < 5000);
    drop(outside);
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn profile_records_window_work_without_a_history_cache_or_same_window_key_reads() {
    use glory_render::profile::{Phase, Profile};
    let (app, _observed) = app();
    let clock = Rc::new(Cell::new(0u64));
    let time = clock.clone();
    let profile = Profile::new(move || {
        let next = time.get() + 1;
        time.set(next);
        std::time::Duration::from_nanos(next)
    });
    app.set_profile(Some(profile.clone()));
    let position = app.scope().cage(0.);
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000usize,
            move |i| {
                keys.set(keys.get() + 1);
                i
            },
            |_, key| text().content(format!("row:{key}")),
        )
        .position(position)
        .height(600.)
        .row_height(24.)
        .overscan(4),
    )
    .unwrap();
    assert!(profile.snapshot().sample(Phase::Build).calls > 0);
    // Leave the exact top edge first: a fractional first row legitimately
    // adds one partially visible row. Both following positions share a window.
    position.revise(|v| *v = 1.);
    app.flush().unwrap();
    assert!(profile.reset());
    let initial = reads.get();
    position.revise(|v| *v = 2.);
    app.flush().unwrap();
    assert_eq!(reads.get(), initial);
    assert_eq!(profile.snapshot().sample(Phase::Reconcile).units, 0);
    assert_eq!(profile.snapshot().sample(Phase::Commit).calls, 0);
    for step in 1..=100 {
        position.revise(|v| *v = (step * 19_123_000) as f64);
        app.flush().unwrap();
    }
    // Fractional far positions may show 26 rows plus eight overscan rows.
    assert!(profile.snapshot().sample(Phase::Reconcile).units <= 3400);
    assert!(profile.snapshot().sample(Phase::Window).calls >= 100);
    assert!(app.stats().nodes <= 70 && app.runtime_stats().node_slots < 200);
    app.set_profile(None);
    let before = clock.get();
    let snapshot = profile.snapshot();
    position.revise(|v| *v = 0.);
    app.pump().unwrap();
    assert_eq!(clock.get(), before);
    assert_eq!(profile.snapshot(), snapshot);
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}
fn root(app: &App) -> NodeId {
    app.tree().roots()[0]
}
fn rows(app: &App) -> Vec<NodeId> {
    let tree = app.tree();
    let body = tree.node(root(app)).unwrap().children[0];
    tree.node(body).unwrap().children.clone()
}
fn row_index(app: &App, id: NodeId) -> usize {
    let tree = app.tree();
    let child = tree.node(id).unwrap().children[0];
    let Props::Text(props) = &tree.node(child).unwrap().descriptor.props else {
        panic!()
    };
    props.content.strip_prefix("row:").unwrap().parse().unwrap()
}
fn measure_event(height: f64) -> Event {
    Event::Resize {
        size: Rect {
            width: 800.0,
            height,
            ..Rect::default()
        },
        viewport: Rect::default(),
    }
}

#[test]
fn measured_rows_anchor_fractional_updates_and_keep_metadata_bounded_across_far_jumps() {
    let (app, observed) = app();
    let position = app.scope().cage(0.0);
    let latest = Rc::new(RefCell::new(None));
    let stats = latest.clone();
    app.mount(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(format!("row:{key}")),
        )
        .measure(24.0)
        .measure_limit(48)
        .position(position)
        .inspect(move |value| *stats.borrow_mut() = Some(value)),
    )
    .unwrap();
    let list_root = root(&app);
    app.dispatch(list_root, scroll_event(4802.0)).unwrap();
    let anchor_id = labels(&app)
        .into_iter()
        .find(|(_, label)| label == "row:200")
        .unwrap()
        .0;
    for id in rows(&app) {
        let index = row_index(&app, id);
        observed.borrow_mut().inputs.push(RenderInput::Event {
            node: id,
            event: measure_event(if index < 200 { 31.375 } else { 24.0 }),
        });
    }
    let before = observed.borrow().commits;
    app.pump().unwrap();
    assert_eq!(observed.borrow().commits, before + 1);
    assert_eq!(
        latest.borrow().as_ref().unwrap().logical,
        4802.0 + 4.0 * 7.375
    );
    assert_eq!(
        labels(&app)
            .into_iter()
            .find(|(_, label)| label == "row:200")
            .unwrap()
            .0,
        anchor_id
    );
    for visit in 1..200 {
        position.revise(|value| *value = ((visit * 7919) % 100_000) as f64 * 24_000.0);
        app.flush().unwrap();
        for id in rows(&app) {
            let index = row_index(&app, id);
            observed.borrow_mut().inputs.push(RenderInput::Event {
                node: id,
                event: measure_event(24.125 + (index % 3) as f64),
            });
        }
        app.pump().unwrap();
        let stats = latest.borrow();
        let stats = stats.as_ref().unwrap();
        assert!(stats.measured <= 48.max(stats.range.len()));
        assert!(stats.height_slots <= stats.measured * 29);
        assert!(stats.height_capacity <= 4096);
        assert!(stats.rows <= 35);
        assert!(app.stats().node_slots <= 145);
        assert!(app.stats().handlers <= 38);
    }
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.stats().handlers, 0);
}

#[test]
fn measurements_validate_estimates_ignore_retired_events_and_diagnose_invalid_keys() {
    let (app, observed) = app();
    assert!(
        app.mount(list(100_000_000_usize, |index| index, |_, _| text()).measure(0.001))
            .is_err()
    );
    let count = app.scope().cage(1000_usize);
    let duplicate = app.scope().cage(false);
    app.mount(
        list(
            count,
            move |index| if duplicate.get() { 0 } else { index },
            |_, key| text().content(format!("row:{key}")),
        )
        .measure(24.0),
    )
    .unwrap();
    let retired = rows(&app)[0];
    app.dispatch(root(&app), scroll_event(4802.0)).unwrap();
    observed.borrow_mut().inputs.push(RenderInput::Event {
        node: retired,
        event: measure_event(500.0),
    });
    app.pump().unwrap();
    duplicate.revise(|value| *value = true);
    count.revise(|value| *value = 4);
    assert_eq!(
        app.flush().unwrap_err(),
        glory_ui::Error::List(glory_ui::list::Error::DuplicateKey)
    );
    assert!(labels(&app).is_empty());
    duplicate.revise(|value| *value = false);
    count.revise(|value| *value = 20);
    app.flush().unwrap();
    assert_eq!(labels(&app).len(), 20);
}

#[test]
fn an_incorrect_locate_result_is_a_typed_error_instead_of_scrolling_to_another_key() {
    let (app, _) = app();
    let base = app.scope().cage(0_usize);
    app.mount(
        list(
            10_000_usize,
            move |index| base.get() + index,
            |_, key| text().content(format!("row:{key}")),
        )
        .locate(|_| Some(0)),
    )
    .unwrap();
    app.dispatch(root(&app), scroll_event(4802.0)).unwrap();
    base.revise(|value| *value = 1);
    assert_eq!(
        app.flush().unwrap_err(),
        glory_ui::Error::List(glory_ui::list::Error::InvalidLookup)
    );
}

#[test]
fn handover_restores_an_inset_larger_than_the_old_estimate_after_coalesced_measurement() {
    let (app, observed) = app();
    let latest = Rc::new(RefCell::new(None));
    let stats = latest.clone();
    app.mount(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(format!("row:{key}")),
        )
        .measure(24.0)
        .inspect(move |value| *stats.borrow_mut() = Some(value)),
    )
    .unwrap();
    let row = rows(&app)[0];
    observed.borrow_mut().inputs.extend([
        RenderInput::Event {
            node: row,
            event: measure_event(200.0),
        },
        RenderInput::Event {
            node: root(&app),
            event: Event::Anchor {
                row,
                within: 150.5,
                viewport: Rect {
                    width: 800.0,
                    height: 600.0,
                    ..Rect::default()
                },
            },
        },
    ]);
    app.pump().unwrap();
    assert_eq!(latest.borrow().as_ref().unwrap().logical, 150.5);
    let foreign = glory_render::NodeIds::new(glory_render::RuntimeId::new()).allocate();
    assert_eq!(
        app.dispatch(
            root(&app),
            Event::Anchor {
                row: foreign,
                within: 0.0,
                viewport: Rect {
                    width: 800.0,
                    height: 600.0,
                    ..Rect::default()
                }
            }
        )
        .unwrap_err(),
        glory_ui::Error::Render(glory_render::Error::ForeignRuntime)
    );
}

#[test]
fn suspended_lists_do_not_wait_for_scroll_actions_the_host_never_applied() {
    let (app, observed) = app();
    let mounted = list(
        1_000_000_usize,
        |index| index,
        |_, key| text().content(format!("row:{key}")),
    )
    .position(24_000.0)
    .mount(app.scope())
    .unwrap();
    let id = mounted.roots()[0];
    mounted.suspend().unwrap();
    app.flush().unwrap();
    assert!(!app.scope().is_active(id).unwrap());
    assert!(observed.borrow().actions.is_empty());
    mounted.place_before(None).unwrap();
    app.flush().unwrap();
    app.dispatch(
        id,
        Event::Resize {
            size: Rect::default(),
            viewport: Rect {
                width: 800.0,
                height: 599.0,
                ..Rect::default()
            },
        },
    )
    .unwrap();
    assert!(app.scope().is_active(id).unwrap());
    assert!(
        matches!(observed.borrow().actions.as_slice(), [Action::ScrollTo { y, .. }] if *y == 24_000.0)
    );
}

#[test]
fn locate_preserves_the_same_row_through_large_prepend_and_anchor_deletion() {
    let (app, observed) = app();
    // A compact indexed source represents [base, base+count), no vector scan.
    let base = app.scope().cage(1000_usize);
    let count = app.scope().cage(10_000_usize);
    let latest = Rc::new(RefCell::new(None));
    let stats = latest.clone();
    app.mount(
        list(
            count,
            move |index| base.get() + index,
            |_, key| text().content(format!("row:{key}")),
        )
        .locate(move |key| {
            key.checked_sub(base.get())
                .filter(|index| *index < count.get())
        })
        .inspect(move |value| *stats.borrow_mut() = Some(value)),
    )
    .unwrap();
    app.dispatch(root(&app), scroll_event(4802.0)).unwrap();
    let old = labels(&app)
        .into_iter()
        .find(|(_, label)| label == "row:1200")
        .unwrap()
        .0;
    base.revise(|value| *value = 0);
    count.revise(|value| *value += 1000);
    app.flush().unwrap();
    assert_eq!(latest.borrow().as_ref().unwrap().logical, 28_802.0);
    assert_eq!(
        labels(&app)
            .into_iter()
            .find(|(_, label)| label == "row:1200")
            .unwrap()
            .0,
        old
    );
    // Remove all keys through the anchor. Its successor survives and becomes top.
    base.revise(|value| *value = 1201);
    count.revise(|value| *value -= 1201);
    app.flush().unwrap();
    assert_eq!(latest.borrow().as_ref().unwrap().logical, 0.0);
    assert_eq!(labels(&app)[0].1, "row:1201");
    // Shrinking to zero must not read old-range indices from the source.
    observed.borrow_mut().mutations.clear();
    count.revise(|value| *value = 0);
    app.flush().unwrap();
    assert!(labels(&app).is_empty());
}
fn labels(app: &App) -> Vec<(NodeId, String)> {
    let tree = app.tree();
    let mut pending = tree.roots().to_vec();
    let mut labels = Vec::new();
    while let Some(id) = pending.pop() {
        let node = tree.node(id).unwrap();
        pending.extend(node.children.iter().rev().copied());
        if let Props::Text(props) = &node.descriptor.props {
            labels.push((id, props.content.clone()));
        }
    }
    labels
}

#[test]
fn public_list_mounts_only_visible_rows_and_coalesces_a_tick_of_scroll_events() {
    let (app, observed) = app();
    let keys = Rc::new(Cell::new(0));
    let key_reads = keys.clone();
    let made = Rc::new(Cell::new(0));
    let builds = made.clone();
    app.mount(list(
        100_000_000_usize,
        move |index| {
            key_reads.set(key_reads.get() + 1);
            index
        },
        move |index, _| {
            builds.set(builds.get() + 1);
            text().content(index.map(|index| format!("row:{index}")))
        },
    ))
    .unwrap();
    let id = root(&app);
    assert_eq!(labels(&app).len(), 29);
    assert_eq!(keys.get(), 29);
    assert_eq!(
        app.tree().node(id).unwrap().descriptor.role,
        Some(Role::List)
    );
    let body = app.tree().node(id).unwrap().children[0];
    assert_eq!(
        app.tree().node(body).unwrap().descriptor.style.height,
        Some(glory_style::Length::Px(1_000_000.0))
    );
    let commits = observed.borrow().commits;
    let reads = keys.get();
    for index in 1..=200 {
        observed.borrow_mut().inputs.push(RenderInput::Event {
            node: id,
            event: scroll_event(index as f64 * 24.0),
        });
    }
    app.pump().unwrap();
    assert_eq!(observed.borrow().commits, commits + 1);
    assert_eq!(keys.get() - reads, 33);
    assert_eq!(labels(&app).len(), 33);
    assert!(labels(&app).iter().any(|(_, label)| label == "row:200"));
    app.dispatch(id, scroll_event(4801.0)).unwrap();
    let reads = keys.get();
    let builds = made.get();
    observed.borrow_mut().mutations.clear();
    app.dispatch(id, scroll_event(4802.0)).unwrap();
    assert_eq!(keys.get(), reads);
    assert_eq!(made.get(), builds);
    assert!(observed.borrow().mutations.is_empty());
    assert!(observed.borrow().actions.is_empty());
}

#[test]
fn data_changes_after_pixel_scroll_still_reconcile_keys_and_preserve_retained_rows() {
    let (app, _) = app();
    let source = app.scope().cage((0_u64..1000).collect::<Vec<_>>());
    let count = source.map(Vec::len).with_partial_eq();
    let keys_source = source;
    app.mount(list(
        count,
        move |index| keys_source.with(|values| values[index]),
        move |_, key| text().content(format!("key:{key}")),
    ))
    .unwrap();
    let id = root(&app);
    app.dispatch(id, scroll_event(4801.0)).unwrap();
    app.dispatch(id, scroll_event(4802.0)).unwrap();
    let old = labels(&app)
        .into_iter()
        .find(|(_, text)| text == "key:200")
        .unwrap()
        .0;
    source.revise(|values| values[196..230].reverse());
    app.flush().unwrap();
    let new = labels(&app)
        .into_iter()
        .find(|(_, text)| text == "key:200")
        .unwrap()
        .0;
    assert_eq!(old, new);
    assert!(
        labels(&app).iter().map(|(_, text)| text).eq((196..230)
            .rev()
            .map(|index| format!("key:{index}"))
            .collect::<Vec<_>>()
            .iter())
    );
}

#[test]
fn segmented_scroll_seeks_huge_offsets_without_scaling_wheel_motion_or_rebase_loops() {
    let (app, observed) = app();
    let position = app.scope().cage(0.0);
    let logical = Rc::new(RefCell::new(Vec::new()));
    let values = logical.clone();
    app.mount(
        list(
            1_000_000_000_usize,
            |index| index,
            |index, _| text().content(index.map(|index| index.to_string())),
        )
        .position(position)
        .on_scroll(move |value| values.borrow_mut().push(value)),
    )
    .unwrap();
    let id = root(&app);
    position.revise(|value| *value = 12_000_000_000.0);
    app.flush().unwrap();
    let Action::ScrollTo { y: physical, .. } = observed.borrow().actions.last().cloned().unwrap()
    else {
        panic!()
    };
    assert!(physical > 0.0 && physical < 1_000_000.0);
    assert!(labels(&app).iter().any(|(_, label)| label == "500000000"));
    let actions = observed.borrow().actions.len();
    app.dispatch(id, scroll_event(physical)).unwrap();
    assert_eq!(observed.borrow().actions.len(), actions);
    app.dispatch(id, scroll_event(physical + 1.0)).unwrap();
    assert_eq!(logical.borrow().last().copied(), Some(12_000_000_001.0));
    assert_eq!(observed.borrow().actions.len(), actions); // No recenter on each pixel.
    position.revise(|value| *value = 24_000_000_000.0);
    app.flush().unwrap();
    assert_eq!(labels(&app).last().unwrap().1, "999999999");
    let actions = observed.borrow().actions.len();
    app.dispatch(
        id,
        Event::Resize {
            size: Rect::default(),
            viewport: Rect {
                width: 800.0,
                height: 500.0,
                ..Rect::default()
            },
        },
    )
    .unwrap();
    assert!(observed.borrow().actions.len() <= actions + 1);
}

#[test]
fn component_owners_tasks_handlers_and_model_slots_stay_bounded_across_far_jumps() {
    let (app, observed) = app();
    let position = app.scope().cage(0.0);
    let created = Rc::new(Cell::new(0));
    let builds = created.clone();
    let cancelled = Rc::new(Cell::new(0));
    let aborts = cancelled.clone();
    app.mount(
        list(
            1_000_000_usize,
            |index| index,
            move |index, _| {
                builds.set(builds.get() + 1);
                let aborts = aborts.clone();
                component(move |scope| {
                    let task = scope.task();
                    let aborts = aborts.clone();
                    task.on_cancel(move || aborts.set(aborts.get() + 1))?;
                    task.detach();
                    text()
                        .content(index.map(|index| index.to_string()))
                        .show_in(scope)
                })
            },
        )
        .position(position),
    )
    .unwrap();
    let id = root(&app);
    for index in (0..1_000_000).step_by(10007) {
        position.revise(|position| *position = index as f64 * 24.0);
        app.flush().unwrap();
        let requested = observed.borrow().actions.last().cloned();
        if let Some(Action::ScrollTo { y, .. }) = requested {
            app.dispatch(id, scroll_event(y)).unwrap();
        }
        let rows = labels(&app).len();
        let ui = app.stats();
        let runtime = app.runtime_stats();
        assert!(rows <= 34);
        assert!(ui.nodes <= 70 && ui.node_slots <= 70);
        assert_eq!(ui.handlers, 5); // Scroll/Resize/Anchor, focus, navigation.
        assert_eq!(runtime.tasks, rows);
        assert!(runtime.node_slots <= 220);
        assert!(runtime.owner_slots <= 180);
        assert_eq!(created.get() - cancelled.get(), rows);
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.stats().handlers, 0);
    assert_eq!(app.runtime_stats().tasks, 0);
    assert_eq!(created.get(), cancelled.get());
}

#[test]
fn row_maps_of_global_application_data_do_not_accumulate_at_the_root_owner() {
    let (app, observed) = app();
    let position = app.scope().cage(0.0);
    let shared = app.scope().cage(1);
    app.mount(
        list(
            1_000_000_usize,
            |index| index,
            move |_, key| text().content(shared.map(move |value| format!("{key}:{value}"))),
        )
        .position(position),
    )
    .unwrap();
    for index in (0..1_000_000).step_by(10007) {
        position.revise(|position| *position = index as f64 * 24.0);
        app.flush().unwrap();
        assert!(app.runtime_stats().nodes <= 180);
        observed.borrow_mut().mutations.clear();
        observed.borrow_mut().actions.clear();
    }
    app.clear().unwrap();
    assert_eq!(app.runtime_stats().nodes, 2);
    assert_eq!(app.runtime_stats().edges, 0);
}

#[test]
fn focused_virtual_rows_and_their_tasks_survive_far_scrolls_then_release_on_blur() {
    let (app, observed) = app();
    let position = app.scope().cage(0.0);
    let latest = Rc::new(RefCell::new(None));
    let inspect = latest.clone();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    let created = Rc::new(Cell::new(0));
    let builds = created.clone();
    let cancelled = Rc::new(Cell::new(0));
    let aborts = cancelled.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            move |_, key| {
                builds.set(builds.get() + 1);
                let aborts = aborts.clone();
                component(move |scope| {
                    let task = scope.task();
                    let aborts = aborts.clone();
                    task.on_cancel(move || aborts.set(aborts.get() + 1))?;
                    task.detach();
                    glory_ui::input()
                        .value(format!("draft:{key}"))
                        .show_in(scope)
                })
            },
        )
        .height(240.0)
        .overscan(2)
        .position(position)
        .inspect(move |stats| *inspect.borrow_mut() = Some(stats)),
    )
    .unwrap();
    let editor = |row: NodeId| app.tree().node(row).unwrap().children[0];
    let focused = editor(rows(&app)[2]);
    app.dispatch(focused, Event::FocusChanged(true)).unwrap();
    for step in 1..150 {
        let before = reads.get();
        position.revise(|position| *position = ((step * 7919) % 100_000) as f64 * 24_000.0);
        app.flush().unwrap();
        let latest = latest.borrow();
        let stats = latest.as_ref().unwrap();
        assert_eq!(stats.pinned, 1);
        assert_eq!(stats.rows, stats.range.len() + 1);
        assert!(stats.rows <= 16);
        assert!(reads.get() - before <= stats.range.len() + 1);
        assert_eq!(app.runtime_stats().tasks, stats.rows);
        assert_eq!(created.get() - cancelled.get(), stats.rows);
        assert!(app.stats().node_slots <= 36);
        assert_eq!(app.stats().handlers, 5);
        assert!(
            matches!(&app.tree().node(focused).unwrap().descriptor.props, Props::Input(value) if value.value == "draft:2")
        );
        observed.borrow_mut().actions.clear();
        observed.borrow_mut().mutations.clear();
    }
    // Several native transitions in one tick preserve the final A target.
    let other = editor(*rows(&app).last().unwrap());
    observed.borrow_mut().inputs.extend([
        RenderInput::Event {
            node: focused,
            event: Event::FocusChanged(false),
        },
        RenderInput::Event {
            node: other,
            event: Event::FocusChanged(true),
        },
        RenderInput::Event {
            node: other,
            event: Event::FocusChanged(false),
        },
        RenderInput::Event {
            node: focused,
            event: Event::FocusChanged(true),
        },
    ]);
    app.pump().unwrap();
    assert!(app.tree().node(focused).is_ok());
    assert_eq!(latest.borrow().as_ref().unwrap().pinned, 1);
    app.dispatch(other, Event::FocusChanged(true)).unwrap();
    assert!(app.tree().node(focused).is_err());
    app.dispatch(other, Event::FocusChanged(false)).unwrap();
    position.revise(|position| *position = 0.0);
    app.flush().unwrap();
    assert!(app.tree().node(other).is_err());
    assert_eq!(latest.borrow().as_ref().unwrap().pinned, 0);
    app.clear().unwrap();
    assert_eq!(app.runtime_stats().tasks, 0);
    assert_eq!(created.get(), cancelled.get());
    assert_eq!(app.stats().handlers, 0);
}

#[test]
fn cached_keyed_editor_updates_a_distant_focus_pin_and_retires_its_target_on_blur() {
    let (app, observed) = app();
    let position = app.scope().cage(0.);
    let source = app.scope().cage(std::collections::HashMap::from([(
        0usize,
        "draft🙂".to_owned(),
    )]));
    let latest = Rc::new(RefCell::new(None));
    let inspect = latest.clone();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            move |_, key| {
                component(move |scope| {
                    let item = source.entry(key);
                    glory_ui::input()
                        .value(
                            scope
                                .bond(move || item.try_get().unwrap_or_default())
                                .with_partial_eq(),
                        )
                        .show_in(scope)
                })
            },
        )
        .height(240.)
        .overscan(2)
        .row_limit(20)
        .position(position)
        .inspect(move |s| *inspect.borrow_mut() = Some(s)),
    )
    .unwrap();
    let focused = app.tree().node(rows(&app)[0]).unwrap().children[0];
    app.dispatch(focused, Event::FocusChanged(true)).unwrap();
    let writer_owner = app.runtime().owner();
    let writer = writer_owner.enter(|| source.entry(0));
    for step in 1..=300 {
        position.revise(|p| *p = ((step * 7919) % 100_000) as f64 * 24_000.);
        app.flush().unwrap();
        let stats = latest.borrow().clone().unwrap();
        assert_eq!(stats.pinned, 1);
        assert!(stats.rows <= 21);
        let before = app.runtime_stats();
        let key_reads = reads.get();
        writer.revise(|v| *v = format!("draft🙂{step}"));
        app.flush().unwrap();
        writer.revise(|v| v.push('!'));
        app.flush().unwrap();
        let runtime = app.runtime_stats();
        assert_eq!(runtime.field_target_builds - before.field_target_builds, 1);
        assert_eq!(runtime.field_target_hits - before.field_target_hits, 1);
        assert_eq!(
            (runtime.field_target_caches, runtime.field_target_keys),
            (1, 3)
        );
        assert_eq!(reads.get(), key_reads);
        assert_eq!(runtime.entry_keys, stats.rows);
        assert!(
            runtime.field_slots <= 42 && runtime.node_slots <= 220 && runtime.owner_slots <= 220
        );
        assert!(
            matches!(&app.tree().node(focused).unwrap().descriptor.props,Props::Input(p) if p.value==format!("draft🙂{step}!"))
        );
        observed.borrow_mut().actions.clear();
        observed.borrow_mut().mutations.clear();
    }
    drop(writer_owner);
    assert!(!writer.is_alive());
    assert_eq!(app.runtime_stats().field_target_caches, 0);
    app.dispatch(focused, Event::FocusChanged(false)).unwrap();
    assert!(app.tree().node(focused).is_err());
    assert_eq!(latest.borrow().as_ref().unwrap().pinned, 0);
    assert_eq!(app.runtime_stats().entry_keys, rows(&app).len());
    app.clear().unwrap();
    assert_eq!(
        (
            app.runtime_stats().fields,
            app.runtime_stats().entry_keys,
            app.runtime_stats().field_target_caches,
            app.runtime_stats().field_target_keys
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(source.with(|m| m.len()), 1);
    assert!(reads.get() < 8000);
}

#[test]
fn focused_keys_relocate_and_disappear_without_a_full_source_scan() {
    let (app, _) = app();
    let prefix = app.scope().cage(0_usize);
    let removed = app.scope().cage(false);
    let position = app.scope().cage(0.0);
    let count = prefix.map(|prefix| 100_000_000 + prefix);
    let keys = move |index: usize| {
        let key = index as i64 - prefix.get() as i64;
        if key == 2 && removed.get() {
            -1_000_000
        } else {
            key
        }
    };
    app.mount(
        list(count, keys, |index, key| {
            glory_ui::input().value(index.map(move |index| format!("{key}:{index}")))
        })
        .locate(move |key| {
            if *key == 2 && removed.get() {
                None
            } else {
                usize::try_from(*key + prefix.get() as i64).ok()
            }
        })
        .position(position),
    )
    .unwrap();
    let target = app.tree().node(rows(&app)[2]).unwrap().children[0];
    app.dispatch(target, Event::FocusChanged(true)).unwrap();
    position.revise(|position| *position = 1_200_000_000.0);
    app.flush().unwrap();
    prefix.revise(|prefix| *prefix = 10_000);
    app.flush().unwrap();
    assert!(
        matches!(&app.tree().node(target).unwrap().descriptor.props, Props::Input(value) if value.value == "2:10002")
    );
    // This source edit changes no visible key or length; the single pinned
    // key's own dependency must still invalidate the controller.
    removed.revise(|removed| *removed = true);
    app.flush().unwrap();
    assert!(app.tree().node(target).is_err());
    assert!(rows(&app).len() <= 34);
}

#[test]
fn logical_navigation_batches_keys_and_reaches_the_last_of_a_hundred_million_rows() {
    use glory_render::{FocusMode, Navigation};
    let (app, observed) = app();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            |_, key| text().content(format!("row:{key}")),
        )
        .height(240.0)
        .overscan(2)
        .name("Entries"),
    )
    .unwrap();
    let root = root(&app);
    assert_eq!(
        app.tree().node(root).unwrap().descriptor.focus,
        FocusMode::TabStop
    );
    let navigate = |from, command| {
        app.dispatch(from, Event::Navigate { from, command })
            .unwrap();
        observed
            .borrow()
            .actions
            .iter()
            .rev()
            .find_map(|action| {
                if let Action::Focus(node) = action {
                    Some(*node)
                } else {
                    None
                }
            })
            .unwrap()
    };
    let first = navigate(root, Navigation::Next);
    assert_eq!(row_index(&app, first), 0);
    app.dispatch(first, Event::FocusChanged(true)).unwrap();
    observed
        .borrow_mut()
        .inputs
        .extend((0..35).map(|_| RenderInput::Event {
            node: first,
            event: Event::Navigate {
                from: first,
                command: Navigation::Next,
            },
        }));
    app.pump().unwrap();
    let current = observed
        .borrow()
        .actions
        .iter()
        .rev()
        .find_map(|action| {
            if let Action::Focus(node) = action {
                Some(*node)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(row_index(&app, current), 35);
    app.dispatch(current, Event::FocusChanged(true)).unwrap();
    assert!(app.tree().node(first).is_err());
    let page = navigate(current, Navigation::PageNext);
    assert_eq!(row_index(&app, page), 45);
    let before = reads.get();
    let last = navigate(page, Navigation::Last);
    assert!(reads.get() - before <= 20);
    let item = app.tree().node(last).unwrap().descriptor.item.unwrap();
    assert_eq!(item.index, 99_999_999);
    assert_eq!(item.count, Some(100_000_000));
    assert_eq!(navigate(last, Navigation::Next), last);
    assert_eq!(row_index(&app, navigate(last, Navigation::First)), 0);
    assert!(rows(&app).len() <= 15);
    assert!(
        app.dispatch(
            root,
            Event::Navigate {
                from: last,
                command: Navigation::First
            }
        )
        .is_err()
    );
}

#[test]
fn item_metadata_updates_even_when_data_count_changes_without_window_keys_changing() {
    let (app, _) = app();
    let count = app.scope().cage(100_000_000_usize);
    app.mount(list(
        count,
        |index| index,
        |_, key| text().content(format!("row:{key}")),
    ))
    .unwrap();
    let first = rows(&app)[0];
    count.revise(|count| *count += 1);
    app.flush().unwrap();
    assert_eq!(rows(&app)[0], first);
    assert_eq!(
        app.tree()
            .node(first)
            .unwrap()
            .descriptor
            .item
            .unwrap()
            .count,
        Some(100_000_001)
    );
}

#[test]
fn reveal_retains_a_distant_editor_and_does_not_emit_a_focus_action() {
    use glory_render::{Navigation, Props};
    for measured in [false, true] {
        let (app, observed) = app();
        let position = app.scope().cage(0.0);
        let reads = Rc::new(Cell::new(0));
        let keys = reads.clone();
        let latest = Rc::new(RefCell::new(None));
        let stats = latest.clone();
        let source = list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            |_, key| glory_ui::input().value(format!("draft {key}")),
        )
        .height(180.0)
        .overscan(2)
        .position(position)
        .locate(|key| Some(*key))
        .inspect(move |v| *stats.borrow_mut() = Some(v));
        app.mount(if measured {
            source.measure(24.0)
        } else {
            source.row_height(24.0)
        })
        .unwrap();
        let row = rows(&app)[0];
        let editor = app.tree().node(row).unwrap().children[0];
        app.dispatch(editor, Event::FocusChanged(true)).unwrap();
        position.revise(|p| *p = 90_000_000.0 * 24.0);
        app.flush().unwrap();
        assert_eq!(latest.borrow().as_ref().unwrap().pinned, 1);
        let before = reads.get();
        observed.borrow_mut().actions.clear();
        app.dispatch(
            row,
            Event::Navigate {
                from: row,
                command: Navigation::Reveal,
            },
        )
        .unwrap();
        assert!(latest.borrow().as_ref().unwrap().range.contains(&0));
        assert_eq!(latest.borrow().as_ref().unwrap().pinned, 0);
        assert!(reads.get() - before < 50);
        assert!(
            observed
                .borrow()
                .actions
                .iter()
                .all(|action| !matches!(action, Action::Focus(_)))
        );
        assert!(
            matches!(&app.tree().node(editor).unwrap().descriptor.props, Props::Input(p) if p.value == "draft 0")
        );
        assert!(rows(&app).contains(&row) && app.stats().nodes < 100);
    }
}

#[test]
fn reveal_requires_a_row_but_not_focus_capability_or_a_full_source_search() {
    use glory_render::Navigation;
    let (app, observed) = app();
    observed.borrow_mut().no_focus = true;
    let position = app.scope().cage(90_000_000.0 * 24.0);
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    app.mount(
        list(
            100_000_000usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            |_, key| text().content(format!("row:{key}")),
        )
        .position(position)
        .height(180.0)
        .row_height(24.0)
        .overscan(2),
    )
    .unwrap();
    let root = root(&app);
    let before = reads.get();
    observed.borrow_mut().actions.clear();
    app.dispatch(
        root,
        Event::Navigate {
            from: root,
            command: Navigation::Reveal,
        },
    )
    .unwrap();
    assert_eq!(reads.get(), before);
    assert!(observed.borrow().actions.is_empty());
    let row = *rows(&app).last().unwrap();
    app.dispatch(
        row,
        Event::Navigate {
            from: row,
            command: Navigation::Reveal,
        },
    )
    .unwrap();
    assert!(
        observed
            .borrow()
            .actions
            .iter()
            .any(|action| matches!(action, Action::ScrollTo { .. }))
    );
    assert!(
        observed
            .borrow()
            .actions
            .iter()
            .all(|action| !matches!(action, Action::Focus(_)))
    );
    assert!(reads.get() - before < 50);
}

#[test]
fn a_batched_reveal_uses_the_actual_origin_instead_of_the_navigation_cursor() {
    use glory_render::Navigation;
    let (app, observed) = app();
    app.mount(list(
        100_000_000usize,
        |index| index,
        |_, key| text().content(format!("row:{key}")),
    ))
    .unwrap();
    let row = rows(&app)[0];
    observed.borrow_mut().actions.clear();
    observed
        .borrow_mut()
        .inputs
        .extend(
            [Navigation::Last, Navigation::Reveal].map(|command| RenderInput::Event {
                node: row,
                event: Event::Navigate { from: row, command },
            }),
        );
    app.pump().unwrap();
    assert_eq!(rows(&app)[0], row);
    assert!(
        observed
            .borrow()
            .actions
            .iter()
            .all(|action| !matches!(action, Action::Focus(_)))
    );
}

#[test]
fn empty_navigation_and_focus_deletion_have_a_stable_collection_fallback() {
    use glory_render::Navigation;
    let (app, observed) = app();
    let count = app.scope().cage(0_usize);
    app.mount(list(
        count,
        |index| index,
        |_, key| text().content(format!("row:{key}")),
    ))
    .unwrap();
    let root = root(&app);
    app.dispatch(
        root,
        Event::Navigate {
            from: root,
            command: Navigation::Last,
        },
    )
    .unwrap();
    assert!(observed.borrow().actions.is_empty());
    count.revise(|count| *count = 1);
    app.flush().unwrap();
    let first = rows(&app)[0];
    app.dispatch(first, Event::FocusChanged(true)).unwrap();
    for command in [
        Navigation::Previous,
        Navigation::Next,
        Navigation::PagePrevious,
        Navigation::PageNext,
    ] {
        app.dispatch(
            first,
            Event::Navigate {
                from: first,
                command,
            },
        )
        .unwrap();
        assert_eq!(
            observed.borrow().actions.last(),
            Some(&Action::Focus(first))
        );
    }
    count.revise(|count| *count = 0);
    app.flush().unwrap();
    assert!(rows(&app).is_empty());
    assert_eq!(observed.borrow().actions.last(), Some(&Action::Focus(root)));
    let memory = App::memory();
    memory
        .mount(list(100_usize, |index| index, |_, _| text()))
        .unwrap();
    let root = memory.tree().roots()[0];
    let before = memory.stats().nodes;
    assert!(matches!(
        memory.dispatch(
            root,
            Event::Navigate {
                from: root,
                command: Navigation::Last
            }
        ),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
    assert_eq!(memory.stats().nodes, before);
}

#[test]
fn page_navigation_makes_progress_over_a_row_taller_than_the_viewport() {
    use glory_render::Navigation;
    let (app, observed) = app();
    app.mount(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(format!("row:{key}")),
        )
        .measure(24.0)
        .height(240.0),
    )
    .unwrap();
    let first = rows(&app)[0];
    app.dispatch(first, measure_event(1200.0)).unwrap();
    app.dispatch(
        first,
        Event::Navigate {
            from: first,
            command: Navigation::PageNext,
        },
    )
    .unwrap();
    let target = observed
        .borrow()
        .actions
        .iter()
        .rev()
        .find_map(|action| {
            if let Action::Focus(id) = action {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(row_index(&app, target), 1);
}

#[test]
fn late_navigation_measurements_do_not_scroll_after_focus_leaves() {
    use glory_render::Navigation;
    let (app, observed) = app();
    app.mount(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| text().content(format!("row:{key}")),
        )
        .measure(24.0)
        .height(240.0),
    )
    .unwrap();
    let first = rows(&app)[0];
    app.dispatch(
        first,
        Event::Navigate {
            from: first,
            command: Navigation::Next,
        },
    )
    .unwrap();
    let target = observed
        .borrow()
        .actions
        .iter()
        .rev()
        .find_map(|action| {
            if let Action::Focus(id) = action {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    app.dispatch(target, Event::FocusChanged(true)).unwrap();
    app.dispatch(target, Event::FocusChanged(false)).unwrap();
    observed.borrow_mut().actions.clear();
    app.dispatch(target, measure_event(900.0)).unwrap();
    assert!(observed.borrow().actions.is_empty());
}

#[test]
fn controlled_navigation_accepts_quantized_scroll_and_preserves_a_racing_one_pixel_delta() {
    use glory_render::{Navigation, ScrollAck, ScrollRequest};
    for racing in [false, true] {
        let (app, observed) = app();
        let position = app.scope().cage(0.);
        let latest = Rc::new(RefCell::new(None));
        let inspected = latest.clone();
        app.mount(
            list(
                100_000_000_usize,
                |i| i,
                |_, key| text().content(format!("row:{key}")),
            )
            .height(240.)
            .measure(24.)
            .position(position)
            .on_scroll(move |value| position.revise(|p| *p = value))
            .inspect(move |stats| *inspected.borrow_mut() = Some(stats)),
        )
        .unwrap();
        let root = root(&app);
        app.dispatch(
            root,
            Event::Navigate {
                from: root,
                command: Navigation::Last,
            },
        )
        .unwrap();
        let target = *rows(&app).last().unwrap();
        app.dispatch(target, Event::FocusChanged(true)).unwrap();
        app.dispatch(target, measure_event(24.)).unwrap();
        let (requested, projected) = observed
            .borrow()
            .actions
            .iter()
            .rev()
            .find_map(|action| match action {
                Action::ScrollTo { y, .. } => Some((*y, latest.borrow().as_ref().unwrap().logical)),
                _ => None,
            })
            .unwrap();
        let actual = requested - 0.625;
        let ack = ScrollAck {
            requested: ScrollRequest {
                x: 0.,
                y: requested,
            },
            x: 0.,
            y: actual,
        };
        let event = |y| Event::Scroll {
            x: 0.,
            y,
            viewport: Rect {
                width: 800.,
                height: 240.,
                ..Default::default()
            },
            ack: Some(ack),
        };
        let actions = observed.borrow().actions.len();
        app.dispatch(root, event(actual - if racing { 1. } else { 0. }))
            .unwrap();
        assert!((position.get() - (projected - if racing { 1. } else { 0. })).abs() < 0.001);
        assert_eq!(
            observed.borrow().actions.len(),
            actions,
            "accept/readback must not reissue the scroll"
        );
        app.dispatch(target, measure_event(96.)).unwrap();
        let expected = projected + if racing { -1. } else { 72. };
        assert!(
            (position.get() - expected).abs() < 0.001,
            "{racing}: controlled echo or user input lost"
        );
        if !racing {
            app.dispatch(root, event(actual - 1.)).unwrap();
            let user = position.get();
            assert!((user - (expected - 1.)).abs() < 0.001);
            app.dispatch(target, measure_event(128.)).unwrap();
            assert!(
                (position.get() - user).abs() < 0.001,
                "old navigation intent survived real movement"
            );
        }
    }
}

#[test]
fn oversized_initial_windows_fail_before_source_access_or_component_creation() {
    use glory_ui::list::{DEFAULT_ROW_LIMIT, Error as ListError};
    for (height, row_height, overscan) in [
        (600.0, 1e-9, 4),
        (600.0, 24.0, usize::MAX),
        (2_400_000_000.0, 24.0, 4),
    ] {
        let app = App::memory();
        let reads = Rc::new(Cell::new(0));
        let keys = reads.clone();
        let created = Rc::new(Cell::new(0));
        let builds = created.clone();
        let result = app.mount(
            list(
                100_000_000_usize,
                move |index| {
                    keys.set(keys.get() + 1);
                    index
                },
                move |_, _| {
                    builds.set(builds.get() + 1);
                    text()
                },
            )
            .height(height)
            .row_height(row_height)
            .overscan(overscan),
        );
        assert!(matches!(
            result,
            Err(glory_ui::Error::List(ListError::RowLimit {
                requested: 100_000_000,
                limit: DEFAULT_ROW_LIMIT
            }))
        ));
        assert_eq!(reads.get(), 0);
        assert_eq!(created.get(), 0);
        assert_eq!(app.stats().nodes, 0);
        assert_eq!(app.runtime_stats().nodes, 0);
    }
    let app = App::memory();
    assert!(matches!(
        app.mount(list(1_usize, |index| index, |_, _| text()).row_limit(0)),
        Err(glory_ui::Error::List(ListError::InvalidSize))
    ));
    // Preflight uses the actual initial position, not a larger middle window.
    app.mount(
        list(100_000_000_usize, |index| index, |_, _| text())
            .row_limit(29)
            .position(2_400_000_000.0),
    )
    .unwrap();
    assert_eq!(rows(&app).len(), 29);
}

#[test]
fn an_oversized_resize_releases_rows_and_tasks_and_a_valid_resize_recovers() {
    use glory_ui::list::Error as ListError;
    let (app, observed) = app();
    let reads = Rc::new(Cell::new(0));
    let keys = reads.clone();
    let created = Rc::new(Cell::new(0));
    let builds = created.clone();
    let cancelled = Rc::new(Cell::new(0));
    let aborts = cancelled.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |index| {
                keys.set(keys.get() + 1);
                index
            },
            move |_, _| {
                builds.set(builds.get() + 1);
                let aborts = aborts.clone();
                component(move |scope| {
                    let task = scope.task();
                    let aborts = aborts.clone();
                    task.on_cancel(move || aborts.set(aborts.get() + 1))?;
                    task.detach();
                    text().show_in(scope)
                })
            },
        )
        .height(240.0)
        .row_limit(64),
    )
    .unwrap();
    let root = root(&app);
    app.dispatch(rows(&app)[0], Event::FocusChanged(true))
        .unwrap();
    let before_reads = reads.get();
    let before_created = created.get();
    let resize = |height| Event::Resize {
        size: Rect::default(),
        viewport: Rect {
            width: 800.0,
            height,
            ..Rect::default()
        },
    };
    assert!(matches!(
        app.dispatch(root, resize(1_000_000.0)),
        Err(glory_ui::Error::List(ListError::RowLimit { limit: 64, .. }))
    ));
    assert!(rows(&app).is_empty());
    assert_eq!(app.runtime_stats().tasks, 0);
    assert_eq!(created.get(), cancelled.get());
    assert_eq!(reads.get(), before_reads + 1); // Existing focused key only; no new window keys.
    assert_eq!(created.get(), before_created);
    assert_eq!(observed.borrow().actions.last(), Some(&Action::Focus(root)));
    app.dispatch(root, resize(240.0)).unwrap();
    assert!(!rows(&app).is_empty());
    assert_eq!(app.runtime_stats().tasks, rows(&app).len());
    app.clear().unwrap();
    assert_eq!(created.get(), cancelled.get());
}
