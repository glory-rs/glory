//! Runnable browser acceptance fixture. The application itself uses only the
//! public Rust builders; JS exports expose controls and counters to the harness.
#[cfg(target_arch = "wasm32")]
mod adaptive;
#[cfg(target_arch = "wasm32")]
mod css_client;
pub mod css_form;
#[cfg(target_arch = "wasm32")]
mod entries_client;
pub mod entries_form;
#[cfg(target_arch = "wasm32")]
mod fields_client;
pub mod fields_form;
#[cfg(target_arch = "wasm32")]
mod flex_client;
pub mod flex_form;
#[cfg(target_arch = "wasm32")]
mod focus;
#[cfg(target_arch = "wasm32")]
mod hydrate_client;
pub mod hydration;
#[cfg(target_arch = "wasm32")]
mod input_client;
pub mod input_form;
#[cfg(target_arch = "wasm32")]
mod intrinsic;
#[cfg(target_arch = "wasm32")]
mod owner_client;
pub mod owner_form;
#[cfg(target_arch = "wasm32")]
mod plan_client;
pub mod plan_form;
#[cfg(target_arch = "wasm32")]
mod query_client;
pub mod query_form;
#[cfg(target_arch = "wasm32")]
mod resource_client;
pub mod resource_hydration;
#[cfg(target_arch = "wasm32")]
mod resources;
#[cfg(target_arch = "wasm32")]
mod semantics_client;
pub mod semantics_form;
#[cfg(target_arch = "wasm32")]
mod size_client;
pub mod size_form;
#[cfg(target_arch = "wasm32")]
mod stream_client;
pub mod streaming;
#[cfg(target_arch = "wasm32")]
mod targets;
#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use glory::prelude::*;
    use glory::render::Event;
    use glory::web::WebApp;
    use wasm_bindgen::prelude::*;
    type Result<T = (), E = JsValue> = std::result::Result<T, E>;

    struct State {
        count: Cage<usize>,
        value: Cage<String>,
        checked: Cage<bool>,
        visible: Cage<bool>,
        order: Cage<Vec<usize>>,
        position: Cage<f64>,
        key_reads: Rc<Cell<usize>>,
        scroll: Rc<Cell<f64>>,
        commits: Rc<Cell<usize>>,
        activations: Rc<Cell<usize>>,
        range: Rc<RefCell<Option<glory::ui::Mounted>>>,
    }
    struct Demo {
        web: WebApp,
        state: State,
        profile: RefCell<Option<glory::render::profile::Profile>>,
    }
    thread_local! { static DEMO: RefCell<Option<Demo>> = const { RefCell::new(None) }; }
    thread_local! { static PEER: RefCell<Option<WebApp>> = const { RefCell::new(None) }; }
    thread_local! { static QUERY: RefCell<String> = const { RefCell::new(String::new()) }; }
    fn js(error: impl std::fmt::Display) -> JsValue {
        error.to_string().into()
    }

    #[wasm_bindgen]
    pub fn mount(id: &str) -> Result<(), JsValue> {
        unmount();
        let root = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(id))
            .ok_or_else(|| js("mount element missing"))?;
        let mut state = None;
        let web = glory::web::launch_with(root, |scope| {
            let count = scope.cage(0_usize);
            let value = scope.cage("initial".to_owned());
            let checked = scope.cage(false);
            let visible = scope.cage(true);
            let order = scope.cage(vec![0_usize, 1, 2]);
            let position = scope.cage(0.0);
            let key_reads = Rc::new(Cell::new(0_usize));
            let reads = key_reads.clone();
            let scroll_offset = Rc::new(Cell::new(0.0));
            let scrolled = scroll_offset.clone();
            let commits = Rc::new(Cell::new(0));
            let committed = commits.clone();
            let activations = Rc::new(Cell::new(0));
            let activated = activations.clone();
            let range = Rc::new(RefCell::new(None));
            let mounted_range = range.clone();
            state = Some(State {
                count,
                value,
                checked,
                visible,
                order,
                position,
                key_reads,
                scroll: scroll_offset,
                commits,
                activations,
                range,
            });
            view().class("demo").fill((
                button()
                    .class("increment")
                    .label(count.map(|count| format!("count {count}")))
                    .fill(text().content("icon").class("icon"))
                    .on(activate, move |_| count.revise(|count| *count += 1)),
                input()
                    .class("editor")
                    .value(value)
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|value| *value = next);
                        }
                    })
                    .on(commit, move |_| committed.set(committed.get() + 1)),
                check()
                    .class("check")
                    .label("check label")
                    .checked(checked)
                    .on(checked_changed, move |event| {
                        if let Event::CheckedChanged(next) = event {
                            checked.revise(|checked| *checked = next);
                        }
                    })
                    .on(activate, move |_| activated.set(activated.get() + 1)),
                when(visible, || {
                    group().fill((
                        text().content("first").class("branch-a"),
                        text().content("second").class("branch-b"),
                    ))
                }),
                view().class("keyed").fill(each(
                    order,
                    |item| *item,
                    |item, _| {
                        input()
                            .name(item.map(|item| format!("key {item}")))
                            .value(item.map(|item| format!("item {item}")))
                    },
                )),
                view().class("portal"),
                component(move |scope| {
                    let mounted = group()
                        .fill((
                            input().class("range-editor").value("range"),
                            text().class("range-tail").content("tail"),
                        ))
                        .mount(scope)?;
                    *mounted_range.borrow_mut() = Some(mounted);
                    Ok(())
                }),
                list(
                    100_000_000_usize,
                    move |index| {
                        reads.set(reads.get() + 1);
                        index
                    },
                    |_, key| text().content(format!("row {key}")),
                )
                .height(600.0)
                .row_height(24.0)
                .position(position)
                .on_scroll(move |offset| scrolled.set(offset)),
            ))
        })
        .map_err(js)?;
        DEMO.with(|demo| {
            *demo.borrow_mut() = Some(Demo {
                web,
                state: state.expect("constructed state"),
                profile: RefCell::new(None),
            })
        });
        Ok(())
    }
    #[wasm_bindgen]
    pub fn unmount() {
        crate::css_client::clear();
        crate::entries_client::clear();
        crate::fields_client::clear();
        crate::owner_client::clear();
        crate::query_client::clear();
        crate::input_client::clear();
        crate::plan_client::clear();
        crate::semantics_client::clear();
        crate::size_client::clear();
        crate::flex_client::clear();
        crate::targets::clear();
        crate::intrinsic::clear();
        crate::focus::clear();
        crate::stream_client::clear();
        crate::resource_client::clear();
        crate::resources::clear();
        crate::hydrate_client::clear();
        crate::adaptive::clear();
        PEER.with(|peer| {
            let previous = peer.borrow_mut().take();
            drop(previous);
        });
        DEMO.with(|demo| {
            let previous = demo.borrow_mut().take();
            drop(previous);
        });
    }
    #[wasm_bindgen]
    pub fn mount_peer(nested: bool) -> Result {
        PEER.with(|peer| {
            let previous = peer.borrow_mut().take();
            drop(previous);
        });
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| js("document missing"))?;
        let root = document
            .query_selector(if nested { ".portal" } else { "#peer" })?
            .ok_or_else(|| js("peer element missing"))?;
        let web = glory::web::launch_with(root, |scope| {
            let count = scope.cage(0_usize);
            button()
                .class("peer-button")
                .label(count.map(|count| format!("peer {count}")))
                .on(activate, move |_| count.revise(|count| *count += 1))
        })
        .map_err(js)?;
        PEER.with(|peer| *peer.borrow_mut() = Some(web));
        Ok(())
    }
    fn update(f: impl FnOnce(&State)) -> Result<(), JsValue> {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            f(&demo.state);
            demo.web.app().flush().map_err(js)
        })
    }
    #[wasm_bindgen]
    pub fn profile_start() -> Result {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let profile = demo.web.enable_profile().map_err(js)?;
            demo.profile.replace(Some(profile));
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn settle_probe(limit: usize) -> Result<String> {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let turn = demo.web.app().settle(limit).map_err(js)?;
            Ok(format!(
                "{{\"passes\":{},\"handled\":{},\"pending\":{}}}",
                turn.passes, turn.handled, turn.pending
            ))
        })
    }
    #[wasm_bindgen]
    pub fn profile_stop() -> Result {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            demo.web.app().set_profile(None);
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn profile_reset() -> Result {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let profile = demo.profile.borrow();
            let profile = profile.as_ref().ok_or_else(|| js("profile missing"))?;
            if profile.reset() {
                Ok(())
            } else {
                Err(js("profile epoch exhausted"))
            }
        })
    }
    #[wasm_bindgen]
    pub fn profile_stats() -> Result<String> {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let profile = demo.profile.borrow();
            let profile = profile.as_ref().ok_or_else(|| js("profile missing"))?;
            let rows = profile
                .snapshot()
                .iter()
                .map(|(phase, s)| {
                    format!(
                        "\"{}\":{{\"calls\":{},\"units\":{},\"elapsed_us\":{},\"max_us\":{},\"invalid_clock\":{}}}",
                        phase.name(),
                        s.calls,
                        s.units,
                        s.elapsed.as_secs_f64() * 1e6,
                        s.max.as_secs_f64() * 1e6,
                        s.invalid_clock
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            Ok(format!("{{\"enabled\":{},\"phases\":{{{rows}}}}}", demo.web.app().profile().is_some()))
        })
    }
    #[wasm_bindgen]
    pub fn seek(offset: f64) -> Result<(), JsValue> {
        update(|state| state.position.revise(|value| *value = offset))
    }
    #[wasm_bindgen]
    pub fn dirty_burst(writes: u32) -> Result<String> {
        if !(1..=10_000).contains(&writes) {
            return Err(js("fixture write budget"));
        }
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let app = demo.web.app();
            let before = app.runtime_stats();
            let keys = demo.state.key_reads.get();
            app.runtime()
                .batch(|| {
                    for write in 0..writes {
                        let index = (write as usize * 7919) % (100_000_000 - 100);
                        demo.state.position.revise(|value| *value = index as f64 * 24.);
                    }
                })
                .map_err(js)?;
            app.flush().map_err(js)?;
            let after = app.runtime_stats();
            Ok(format!(
                "{{\"writes\":{writes},\"source_writes\":{},\"marked\":{},\"before_nodes\":{},\"after_nodes\":{},\"key_reads\":{},\"target\":{}}}",
                after.source_writes - before.source_writes,
                after.marked - before.marked,
                before.nodes,
                after.nodes,
                demo.state.key_reads.get() - keys,
                demo.state.position.get()
            ))
        })
    }
    #[wasm_bindgen]
    pub fn set_value(value: &str) -> Result<(), JsValue> {
        update(|state| state.value.revise(|old| *old = value.to_owned()))
    }
    #[wasm_bindgen]
    pub fn set_checked(value: bool) -> Result<(), JsValue> {
        update(|state| state.checked.revise(|old| *old = value))
    }
    #[wasm_bindgen]
    pub fn increment() -> Result<(), JsValue> {
        update(|state| state.count.revise(|value| *value += 1))
    }
    #[wasm_bindgen]
    pub fn toggle() -> Result<(), JsValue> {
        update(|state| state.visible.revise(|value| *value = !*value))
    }
    #[wasm_bindgen]
    pub fn reorder() -> Result<(), JsValue> {
        update(|state| state.order.revise(|value| value.reverse()))
    }
    #[wasm_bindgen]
    pub fn show_range(show: bool) -> Result {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let range = demo.state.range.borrow();
            let range = range.as_ref().ok_or_else(|| js("range missing"))?;
            if show {
                range.place_before(None).map_err(js)?;
            } else {
                range.suspend().map_err(js)?;
            }
            demo.web.app().flush().map_err(js)
        })
    }
    fn editor(app: &App) -> glory::render::NodeId {
        let tree = app.tree();
        let mut pending = tree.roots().to_vec();
        while let Some(id) = pending.pop() {
            let node = tree.node(id).expect("live model node");
            if node
                .descriptor
                .classes
                .iter()
                .any(|class| class == "editor")
            {
                return id;
            }
            pending.extend_from_slice(&node.children);
        }
        panic!("editor missing")
    }
    #[wasm_bindgen]
    pub fn query_editor(selection: bool) -> Result {
        QUERY.with(|result| *result.borrow_mut() = "pending".into());
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let id = editor(demo.web.app());
            use glory::render::{Query, QueryValue};
            demo.web
                .app()
                .request(
                    if selection {
                        Query::Selection(id)
                    } else {
                        Query::Bounds(id)
                    },
                    |reply| {
                        let reply = match reply {
                            Ok(QueryValue::Selection { start, end }) => {
                                format!("{{\"start\":{start},\"end\":{end}}}")
                            }
                            Ok(QueryValue::Bounds(rect)) => {
                                format!("{{\"width\":{},\"height\":{}}}", rect.width, rect.height)
                            }
                            other => format!("{other:?}"),
                        };
                        QUERY.with(|result| *result.borrow_mut() = reply);
                    },
                )
                .map_err(js)?;
            Ok(())
        })
    }
    #[wasm_bindgen]
    pub fn query_result() -> String {
        QUERY.with(|result| result.borrow().clone())
    }
    #[wasm_bindgen]
    pub fn focus_editor() -> Result {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().ok_or_else(|| js("unmounted"))?;
            let app = demo.web.app();
            app.scope()
                .action(glory::render::Action::Focus(editor(app)))
                .map_err(js)?;
            app.flush().map_err(js)
        })
    }
    #[wasm_bindgen]
    pub fn stats() -> String {
        DEMO.with(|demo| { let demo = demo.borrow(); let Some(demo) = demo.as_ref() else { return "null".into() }; let ui = demo.web.app().stats(); let runtime = demo.web.app().runtime_stats();
            let value = js_sys::JSON::stringify(&demo.state.value.get().into()).expect("string JSON");
            format!("{{\"nodes\":{},\"slots\":{},\"handlers\":{},\"graph\":{},\"keys\":{},\"offset\":{},\"count\":{},\"value\":{},\"checked\":{},\"commits\":{},\"activations\":{}}}", ui.nodes, ui.node_slots, ui.handlers, runtime.nodes, demo.state.key_reads.get(), demo.state.scroll.get(), demo.state.count.get(), value.as_string().expect("JSON string"), demo.state.checked.get(), demo.state.commits.get(), demo.state.activations.get())
        })
    }
}
