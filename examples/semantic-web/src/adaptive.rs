//! Measured-height acceptance app: the source is indexed arithmetic, not a
//! vector containing 100 million entries. Business data stays in owned Cages.
use std::cell::RefCell;
use std::rc::Rc;

use glory::prelude::*;
use glory::ui::list::ListStats;
use glory::web::WebApp;
use wasm_bindgen::prelude::*;

type Result<T = ()> = std::result::Result<T, JsValue>;
struct Adaptive {
    web: WebApp,
    base: Cage<usize>,
    count: Cage<usize>,
    position: Cage<f64>,
    changed: Cage<Option<(usize, f64)>>,
    stats: Rc<RefCell<Option<ListStats>>>,
}
thread_local! { static APP: RefCell<Option<Adaptive>> = const { RefCell::new(None) }; }
fn js(error: impl std::fmt::Display) -> JsValue {
    error.to_string().into()
}
pub(crate) fn clear() {
    APP.with(|app| {
        let previous = app.borrow_mut().take();
        drop(previous);
    });
}

#[wasm_bindgen]
pub fn adaptive_mount() -> Result {
    mount(false, 64)
}
#[wasm_bindgen]
pub fn adaptive_mount_limit(limit: usize) -> Result {
    mount(false, limit)
}
/// Same indexed 100-million source, with intrinsic text heights instead of
/// explicit row heights. Used to verify typography-driven Resize measurements.
#[wasm_bindgen]
pub fn adaptive_text_mount() -> Result {
    mount(true, 64)
}
fn mount(text_mode: bool, measure_limit: usize) -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app"))
        .ok_or_else(|| js("app root missing"))?;
    let mut state = None;
    let web = glory::web::launch_with(root, |scope| {
        let base = scope.cage(1_000_000_usize);
        let count = scope.cage(100_000_000_usize);
        let position = scope.cage(0.0);
        let changed = scope.cage(None::<(usize, f64)>);
        let stats = Rc::new(RefCell::new(None));
        let inspected = stats.clone();
        state = Some((base, count, position, changed, stats));
        view().class("adaptive").fill(
            list(
                count,
                move |index| base.get() + index,
                move |_, key| {
                    let style = changed.map(move |changed| {
                        if text_mode {
                            let wraps = changed.is_some_and(|(changed_key, _)| changed_key == key);
                            return Style {
                                font_size: Some(16.0),
                                line_height: Some(20.0),
                                ..Style::new()
                                    .width(Length::Px(160.0))
                                    .font_family([Family::Mono])
                                    .font_weight(if wraps { 700.0 } else { 450.5 })
                                    .white_space(if wraps {
                                        WhiteSpace::PreWrap
                                    } else {
                                        WhiteSpace::NoWrap
                                    })
                            };
                        }
                        let height = changed
                            .filter(|(changed_key, _)| *changed_key == key)
                            .map_or(24.375 + (key % 3) as f64 * 8.0, |(_, height)| height);
                        Style::new().height(Length::Px(height))
                    });
                    let content = if text_mode {
                        format!("row {key} alpha beta gamma delta epsilon zeta eta theta")
                    } else {
                        format!("row {key}")
                    };
                    view()
                        .class("adaptive-content")
                        .style(style)
                        .fill(text().content(content))
                },
            )
            .height(600.0)
            .measure(if text_mode { 20.0 } else { 24.0 })
            .measure_limit(measure_limit)
            .position(position)
            .locate(move |key| {
                key.checked_sub(base.get())
                    .filter(|index| *index < count.get())
            })
            .inspect(move |stats| *inspected.borrow_mut() = Some(stats)),
        )
    })
    .map_err(js)?;
    let (base, count, position, changed, stats) = state.expect("constructed state");
    APP.with(|app| {
        *app.borrow_mut() = Some(Adaptive {
            web,
            base,
            count,
            position,
            changed,
            stats,
        })
    });
    Ok(())
}
fn update(f: impl FnOnce(&Adaptive) -> Result) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("adaptive app missing"))?;
        f(app)?;
        app.web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn adaptive_seek(offset: f64) -> Result {
    update(|app| {
        app.position.revise(|value| *value = offset);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn adaptive_grow(key: usize, height: f64) -> Result {
    update(|app| {
        app.changed.revise(|value| *value = Some((key, height)));
        Ok(())
    })
}
#[wasm_bindgen]
pub fn adaptive_prepend(amount: usize) -> Result {
    update(|app| {
        let base = app
            .base
            .get()
            .checked_sub(amount)
            .ok_or_else(|| js("prepend exceeds fixture key range"))?;
        app.base.revise(|value| *value = base);
        app.count.revise(|value| *value += amount);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn adaptive_drop_before(key: usize) -> Result {
    update(|app| {
        let amount = key
            .checked_sub(app.base.get())
            .filter(|amount| *amount <= app.count.get())
            .ok_or_else(|| js("invalid deletion"))?;
        app.base.revise(|value| *value = key);
        app.count.revise(|value| *value -= amount);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn adaptive_stats() -> String {
    APP.with(|app| {
        let app = app.borrow(); let Some(app) = app.as_ref() else { return "null".into() };
        let stats = app.stats.borrow(); let Some(stats) = stats.as_ref() else { return "null".into() };
        let ui = app.web.app().stats(); let graph = app.web.app().runtime_stats();
        format!("{{\"start\":{},\"end\":{},\"rows\":{},\"measured\":{},\"heightSlots\":{},\"heightCapacity\":{},\"logical\":{},\"total\":{},\"nodes\":{},\"slots\":{},\"handlers\":{},\"graph\":{},\"graphSlots\":{},\"viewportHeight\":{},\"viewportWidth\":{}}}", stats.range.start, stats.range.end, stats.rows, stats.measured, stats.height_slots, stats.height_capacity, stats.logical, stats.total, ui.nodes, ui.node_slots, ui.handlers, graph.nodes, graph.node_slots, stats.viewport.height, stats.viewport.width)
    })
}
