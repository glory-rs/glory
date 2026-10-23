//! Actual image loading/ResizeObserver and controlled navigation acceptance.
use glory::prelude::*;
use glory::ui::list::ListStats;
use glory::web::WebApp;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
type Result<T = ()> = std::result::Result<T, JsValue>;
struct Demo {
    web: WebApp,
    position: Cage<f64>,
    changed: Cage<Option<usize>>,
    stats: Rc<RefCell<Option<ListStats>>>,
    reads: Rc<Cell<usize>>,
    reports: Rc<Cell<usize>>,
}
thread_local! {static APP:RefCell<Option<Demo>>=const{RefCell::new(None)};}
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
pub fn intrinsic_mount() -> Result {
    crate::browser::unmount();
    let root = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("app"))
        .ok_or_else(|| js("app root missing"))?;
    let mut state = None;
    let web = glory::web::launch_with(root, |scope| {
        let position = scope.cage(24_000_007.);
        let changed = scope.cage(None::<usize>);
        let stats = Rc::new(RefCell::new(None));
        let inspect = stats.clone();
        let reads = Rc::new(Cell::new(0));
        let source = reads.clone();
        let reports = Rc::new(Cell::new(0));
        let reported = reports.clone();
        state = Some((position, changed, stats, reads, reports));
        list(
            100_000_000_usize,
            move |index| {
                source.set(source.get() + 1);
                index
            },
            move |_, key| {
                image()
                    .source(
                        changed
                            .map(move |value| {
                                format!(
                                    "/intrinsic-assets/{key}.png?v={}",
                                    usize::from(*value == Some(key))
                                )
                            })
                            .with_partial_eq(),
                    )
                    .class("intrinsic-photo")
                    .alt("photo")
            },
        )
        .name("Photos")
        .height(180.)
        .measure(24.)
        .measure_limit(64)
        .overscan(2)
        .row_limit(32)
        .position(position)
        .on_scroll(move |value| {
            reported.set(reported.get() + 1);
            position.revise(|p| *p = value);
        })
        .inspect(move |value| *inspect.borrow_mut() = Some(value))
    })
    .map_err(js)?;
    let (position, changed, stats, reads, reports) = state.expect("fixture state");
    APP.with(|app| {
        *app.borrow_mut() = Some(Demo {
            web,
            position,
            changed,
            stats,
            reads,
            reports,
        })
    });
    Ok(())
}
#[wasm_bindgen]
pub fn intrinsic_seek(offset: f64) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("unmounted"))?;
        app.position.revise(|p| *p = offset);
        app.web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn intrinsic_replace(key: usize) -> Result {
    APP.with(|app| {
        let app = app.borrow();
        let app = app.as_ref().ok_or_else(|| js("unmounted"))?;
        app.changed.revise(|p| *p = Some(key));
        app.web.app().flush().map_err(js)
    })
}
#[wasm_bindgen]
pub fn intrinsic_stats() -> String {
    APP.with(|app|{
        let app=app.borrow();let Some(app)=app.as_ref() else{return "null".into()};
        let stats=app.stats.borrow();let Some(s)=stats.as_ref() else{return "null".into()};
        let ui=app.web.app().stats();let graph=app.web.app().runtime_stats();
        format!("{{\"start\":{},\"end\":{},\"rows\":{},\"pinned\":{},\"logical\":{},\"echo\":{},\"keys\":{},\"reports\":{},\"measured\":{},\"heightCapacity\":{},\"nodes\":{},\"slots\":{},\"graphSlots\":{}}}",s.range.start,s.range.end,s.rows,s.pinned,s.logical,app.position.get(),app.reads.get(),app.reports.get(),s.measured,s.height_capacity,ui.nodes,ui.node_slots,graph.node_slots)
    })
}
