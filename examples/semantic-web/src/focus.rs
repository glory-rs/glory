//! Real DOM focus/selection and owned task lifetime acceptance for virtual rows.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory::prelude::*;
use glory::ui::list::ListStats;
use glory::web::WebApp;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
#[derive(Default)]
struct Counts {
    created: Cell<usize>,
    dropped: Cell<usize>,
    keys: Cell<usize>,
    max_index: Cell<usize>,
}
struct Demo {
    web: WebApp,
    position: Cage<f64>,
    prefix: Cage<usize>,
    removed: Cage<bool>,
    stats: Rc<RefCell<Option<ListStats>>>,
    counts: Rc<Counts>,
}
thread_local! {
    static APP: RefCell<Option<Demo>> = const { RefCell::new(None) };
    static LAST: RefCell<Option<Rc<Counts>>> = const { RefCell::new(None) };
}
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    APP.with(|app| {
        let old = app.borrow_mut().take();
        drop(old);
    });
}
#[wasm_bindgen]
pub fn focus_mount(adaptive: bool) -> Result {
    mount(adaptive, false)
}
#[wasm_bindgen]
pub fn navigation_mount(adaptive: bool) -> Result {
    mount(adaptive, true)
}
fn mount(adaptive: bool, controls: bool) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .ok_or_else(|| js("root missing"))?;
    let counts = Rc::new(Counts::default());
    LAST.with(|last| *last.borrow_mut() = Some(counts.clone()));
    let stats = Rc::new(RefCell::new(None));
    let inspect = stats.clone();
    let mut state = None;
    let web = glory::web::launch_with(root, |scope| {
        let position = scope.cage(0.0);
        let prefix = scope.cage(0_usize);
        let removed = scope.cage(false);
        let len = scope.bond(move || 100_000_000 + prefix.get() - usize::from(removed.get()));
        state = Some((position, prefix, removed));
        let reads = counts.clone();
        let rows = counts.clone();
        let list = list(
            len,
            move |index| {
                reads.keys.set(reads.keys.get() + 1);
                reads.max_index.set(reads.max_index.get().max(index));
                let key = index as i64 - prefix.get() as i64;
                if removed.get() && key >= 1 {
                    key + 1
                } else {
                    key
                }
            },
            move |index, key| {
                let counts = rows.clone();
                component(move |scope| {
                    counts.created.set(counts.created.get() + 1);
                    let cleanup = counts.clone();
                    scope
                        .owner()
                        .on_cleanup(move || cleanup.dropped.set(cleanup.dropped.get() + 1))?;
                    scope.owner().spawn(std::future::pending())?;
                    let draft = scope.cage(format!("draft {key}"));
                    let row = group().fill((
                        input()
                            .class(format!("edit-{key}"))
                            .value(draft)
                            .style(
                                Style::new()
                                    .height(Length::Px(24.0 + key.rem_euclid(3) as f64 * 0.25)),
                            )
                            .on(value_changed, move |event| {
                                if let Event::ValueChanged { value, .. } = event {
                                    draft.revise(|draft| *draft = value);
                                }
                            }),
                        text()
                            .class(format!("label-{key}"))
                            .content(index.map(move |index| format!("key {key} at {index}"))),
                    ));
                    let row = if controls {
                        row.fill((
                            button().class(format!("action-{key}")).label("Action"),
                            link()
                                .class(format!("link-{key}"))
                                .label("Link")
                                .target("#entry"),
                        ))
                    } else {
                        row
                    };
                    row.show_in(scope)
                })
            },
        )
        .height(240.0)
        .name("Virtual entries")
        .overscan(2)
        .position(position)
        .locate(move |key| {
            if removed.get() && *key == 1 {
                return None;
            }
            usize::try_from(*key + prefix.get() as i64 - i64::from(removed.get() && *key > 1)).ok()
        })
        .inspect(move |stats| *inspect.borrow_mut() = Some(stats));
        let list = if adaptive {
            list.measure(28.0).measure_limit(64)
        } else {
            list.row_height(28.0)
        };
        view()
            .class("focus-panel")
            .style(Style {
                background: Some(glory::style::Color(1, 2, 3, 255)),
                ..Style::new().width(Length::Px(600.0))
            })
            .fill((input().class("outside-focus").value("outside"), list))
    })
    .map_err(js)?;
    let (position, prefix, removed) = state.expect("constructed state");
    APP.with(|app| {
        *app.borrow_mut() = Some(Demo {
            web,
            position,
            prefix,
            removed,
            stats,
            counts,
        })
    });
    Ok(())
}
fn update(f: impl FnOnce(&Demo)) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("unmounted"))?;
        f(app);
        app.web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn focus_jump(position: f64) -> Result {
    update(|app| app.position.revise(|value| *value = position))
}
#[wasm_bindgen]
pub fn focus_prepend() -> Result {
    update(|app| app.prefix.revise(|prefix| *prefix += 10_000))
}
#[wasm_bindgen]
pub fn focus_remove() -> Result {
    update(|app| app.removed.revise(|removed| *removed = true))
}
#[wasm_bindgen]
pub fn focus_stats() -> String {
    APP.with(|app| { let app = app.borrow(); let Some(app) = app.as_ref() else { return "null".into() };
        let stats = app.stats.borrow(); let stats = stats.as_ref().expect("list built");
        let ui = app.web.app().stats(); let graph = app.web.app().runtime_stats();
        format!("{{\"start\":{},\"end\":{},\"rows\":{},\"pinned\":{},\"nodes\":{},\"slots\":{},\"handlers\":{},\"graphSlots\":{},\"tasks\":{},\"created\":{},\"dropped\":{},\"keys\":{},\"maxIndex\":{},\"measured\":{}}}",
            stats.range.start, stats.range.end, stats.rows, stats.pinned, ui.nodes, ui.node_slots, ui.handlers, graph.node_slots, graph.tasks,
            app.counts.created.get(), app.counts.dropped.get(), app.counts.keys.get(), app.counts.max_index.get(), stats.measured)
    })
}
#[wasm_bindgen]
pub fn focus_lifetimes() -> String {
    LAST.with(|last| {
        let last = last.borrow();
        let counts = last.as_ref().expect("mounted once");
        format!(
            "{{\"created\":{},\"dropped\":{}}}",
            counts.created.get(),
            counts.dropped.get()
        )
    })
}
