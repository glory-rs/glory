//! Browser acceptance and measured counts for sparse display Widget islands.
#[cfg(target_arch = "wasm32")]
mod browser {
    use g::{
        Cage, Widget,
        display::App,
        render::{NodeId, Query},
        ui::{Event, EventKind, Scope, WeakScope},
    };
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        rc::Rc,
    };
    use wasm_bindgen::prelude::*;

    struct Handle {
        scope: WeakScope,
        node: NodeId,
        value: Cage<String>,
    }
    type Handles = Rc<RefCell<BTreeMap<usize, Handle>>>;
    struct Island {
        index: usize,
        shared: Cage<usize>,
        handles: Handles,
    }
    impl Widget for Island {
        fn build(&mut self, scope: &Scope) -> g::ui::Result {
            let index = self.index;
            let value = scope.cage("initial".to_owned());
            let input = g::ui::input()
                .class(format!("editor editor-{index}"))
                .value(value)
                .on(EventKind::ValueChanged, move |event| {
                    if let Event::ValueChanged { value: next, .. } = event {
                        value.revise(|current| *current = next);
                    }
                })
                .mount(scope)?;
            self.handles.borrow_mut().insert(
                index,
                Handle {
                    scope: scope.downgrade(),
                    node: input.roots()[0],
                    value,
                },
            );
            let handles = self.handles.clone();
            scope.on_cleanup(move || {
                handles.borrow_mut().remove(&index);
            })?;
            let shared = self.shared;
            let button = g::ui::button()
                .class(format!("global-button-{index}"))
                .label("shared update")
                .on(EventKind::Activate, move |_| {
                    shared.revise(|value| *value += 1)
                })
                .mount(scope)?;
            let text = g::ui::text()
                .class(format!("shared shared-{index}"))
                .content_fn(move || shared.get().to_string())
                .mount(scope)?;
            scope.retain((input, button, text))
        }
    }
    struct Demo {
        app: App,
        raw: Cage<usize>,
        shared: Cage<usize>,
        visible: Cage<bool>,
        rows: g::core::KeyedList<usize, usize>,
        handles: Handles,
        replies: Rc<Cell<usize>>,
        failures: Rc<Cell<usize>>,
    }
    thread_local! { static DEMO: RefCell<Option<Demo>> = const { RefCell::new(None) }; }
    #[wasm_bindgen]
    pub fn island_mount(count: usize) -> Result<(), JsValue> {
        assert!((1..=1000).contains(&count));
        let app = App::at_id("islands").unwrap();
        let scope = app.scope();
        let raw = scope.cage(0usize);
        let shared = scope.cage(0usize);
        let visible = scope.cage(true);
        let handles = Handles::default();
        let rows = scope
            .keyed_list((0..count).collect::<Vec<_>>(), |id| *id)
            .map_err(|error| JsValue::from_str(&format!("island mount: {error:?}")))?;
        let retained = handles.clone();
        app.render(g::view! {
            <div><span class="raw">{raw}</span>
                <Show when={move || visible.get()}>
                    <div class="island-list"><Each each={rows} children={let retained = retained.clone(); move |row| Island {
                        index: *row.key(), shared, handles: retained.clone(),
                    }}/></div>
                </Show>
            </div>
        }).map_err(|error| JsValue::from_str(&format!("island render: {error:?}")))?;
        DEMO.with(|demo| {
            *demo.borrow_mut() = Some(Demo {
                app,
                raw,
                shared,
                visible,
                rows,
                handles,
                replies: Rc::new(Cell::new(0)),
                failures: Rc::new(Cell::new(0)),
            })
        });
        Ok(())
    }
    #[wasm_bindgen]
    pub fn island_raw_updates(iterations: usize) {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            for _ in 0..iterations {
                demo.raw.revise(|value| *value += 1);
                demo.app.flush().unwrap();
            }
        });
    }
    #[wasm_bindgen]
    pub fn island_flush() {
        DEMO.with(|demo| demo.borrow().as_ref().unwrap().app.flush().unwrap());
    }
    #[wasm_bindgen]
    pub fn island_shared_update() {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            demo.shared.revise(|value| *value += 1);
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn island_local_update(index: usize, next: String) {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            let value = demo.handles.borrow().get(&index).unwrap().value;
            value.revise(|value| *value = next);
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn island_value(index: usize) -> String {
        DEMO.with(|demo| {
            demo.borrow()
                .as_ref()
                .unwrap()
                .handles
                .borrow()
                .get(&index)
                .unwrap()
                .value
                .get()
        })
    }
    #[wasm_bindgen]
    pub fn island_dirty_query(index: usize) {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            let (scope, node, value) = {
                let handles = demo.handles.borrow();
                let handle = handles.get(&index).unwrap();
                (handle.scope.upgrade().unwrap(), handle.node, handle.value)
            };
            value.revise(|value| *value = "query barrier".to_owned());
            let replies = demo.replies.clone();
            let failures = demo.failures.clone();
            scope
                .request(Query::Bounds(node), move |result| {
                    replies.set(replies.get() + 1);
                    if result.is_err() {
                        failures.set(failures.get() + 1);
                    }
                })
                .unwrap();
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn island_toggle() {
        DEMO.with(|demo| {
            let demo = demo.borrow();
            let demo = demo.as_ref().unwrap();
            demo.visible.revise(|value| *value = !*value);
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn island_stats() -> String {
        DEMO.with(|demo| {
            let demo = demo.borrow(); let demo = demo.as_ref().unwrap();
            let stats = demo.app.stats(); let runtime = demo.app.runtime().stats();
            format!("{{\"islands\":{},\"island_flushes\":{},\"raw\":{},\"shared\":{},\"handles\":{},\"nodes\":{},\"owners\":{},\"replies\":{},\"failures\":{}}}",
                stats.islands, stats.island_flushes, demo.raw.get(), demo.shared.get(),
                demo.handles.borrow().len(), runtime.nodes, runtime.owners,
                demo.replies.get(), demo.failures.get())
        })
    }
    #[wasm_bindgen]
    pub fn island_close() -> String {
        DEMO.with(|demo| {
            let demo = demo.borrow_mut().take().unwrap();
            demo.app.close().unwrap();
            let stats = demo.app.stats();
            let runtime = demo.app.runtime().stats();
            format!(
                "{{\"islands\":{},\"handles\":{},\"nodes\":{},\"owners\":{}}}",
                stats.islands,
                demo.handles.borrow().len(),
                runtime.nodes,
                runtime.owners
            )
        })
    }
    #[wasm_bindgen]
    pub fn island_reject_extra_and_close() -> Result<String, JsValue> {
        DEMO.with(|slot| {
            let demo = slot.borrow_mut().take().unwrap();
            assert_eq!(demo.app.stats().islands, 64);
            let root = web_sys::window().unwrap().document().unwrap().create_element("div").unwrap();
            // Preserve the exact native error variant at the WebApp boundary.
            let typed = matches!(g::web::launch(root.clone(), g::ui::input()),
                Err(g::ui::Error::Render(g::render::Error::Unsupported("document live App layer budget"))));
            assert!(!root.has_child_nodes() && !root.has_attribute("data-gly-app"));
            // The same admission rule must reject an actual 65th sparse island.
            demo.rows.extend(vec![64]).unwrap();
            let rejected = matches!(demo.app.flush().err(), Some(g::web::view::Error(message))
                if message == "Render(Unsupported(\"document live App layer budget\"))");
            demo.app.close().map_err(|error| JsValue::from_str(&format!("65th close: {error:?}")))?;
            let stats = demo.app.stats(); let runtime = demo.app.runtime().stats();
            Ok(format!("{{\"typedHostRejection\":{typed},\"actualIslandRejection\":{rejected},\"closed\":{{\"islands\":{},\"handles\":{},\"nodes\":{},\"owners\":{}}}}}",
                stats.islands, demo.handles.borrow().len(), runtime.nodes, runtime.owners))
        })
    }
}
fn main() {}
