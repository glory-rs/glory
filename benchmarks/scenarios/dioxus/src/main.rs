#[path = "../../model.rs"]
mod model;
use dioxus::prelude::*;
use model::{Market, Quote};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = scenarioCount)]
    fn count() -> usize;
}
#[derive(Clone, Default)]
struct Registry(Rc<RefCell<HashMap<usize, Signal<Quote>>>>);
impl PartialEq for Registry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
thread_local! { static REGISTRY: RefCell<Option<Registry>> = const { RefCell::new(None) }; }
#[cfg(feature = "rows")]
struct Registration {
    index: usize,
    registry: Registry,
}
#[cfg(feature = "rows")]
impl Drop for Registration {
    fn drop(&mut self) {
        self.registry.0.borrow_mut().remove(&self.index);
    }
}
#[cfg(feature = "rows")]
#[component]
fn QuoteRow(seed: Quote, registry: Registry) -> Element {
    let quote = use_signal(|| seed);
    use_hook(move || {
        registry.0.borrow_mut().insert(seed.index, quote);
        Rc::new(Registration {
            index: seed.index,
            registry,
        })
    });
    render_quote(quote())
}
#[cfg(feature = "rows")]
fn render_quote(quote: Quote) -> Element {
    rsx! { div { role: "row", class: quote.class(),
        span { class: "symbol", "{quote.symbol()}" } span { class: "price", "{quote.price()}" }
        span { class: "quantity", "{quote.quantity()}" } span { class: "amount", "{quote.amount()}" }
    } }
}
#[component]
fn Total(value: Signal<i64>) -> Element {
    rsx! { span { class: "total", "{model::money(value())}" } }
}
#[component]
fn App() -> Element {
    let mut rows = use_signal(Vec::<Quote>::new);
    let mut total = use_signal(|| 0i64);
    let market = use_hook(|| Rc::new(RefCell::new(Market::default())));
    let registry = use_hook(|| {
        let r = Registry::default();
        REGISTRY.with(|slot| *slot.borrow_mut() = Some(r.clone()));
        r
    });
    let buttons = [("mount", "Open portfolio", 0), ("one", "One quote", 1), ("sparse", "1% quotes", 2),
        ("medium", "10% quotes", 3), ("dense", "All quotes", 4), ("burst", "10 market ticks", 5),
        ("select", "Next position", 6), ("clear", "Close portfolio", 7)].into_iter().map(|(class,label,kind)| {
        let market = market.clone(); let registry = registry.clone();
        rsx! { button { class, onclick: move |_| {
            let mut market = market.borrow_mut();
            if kind == 0 { if !market.quotes.is_empty() { return; } rows.set(market.mount(count())); }
            else if kind == 7 { market.clear(); rows.set(Vec::new()); }
            else {
                let n = market.quotes.len();
                let edits = match kind { 1 => market.tick(1, 1), 2 => market.tick(n / 100, 1), 3 => market.tick(n / 10, 1),
                    4 => market.tick(n, 1), 5 => market.tick(n / 100, 10), 6 => market.select(), _ => unreachable!() };
                #[cfg(feature = "rows")]
                for quote in edits { let mut target = registry.0.borrow()[&quote.index]; target.set(quote); }
                #[cfg(not(feature = "rows"))]
                { let _ = &registry; let mut current = rows.write(); for quote in edits { current[quote.index] = quote; } }
            }
            total.set(market.total);
        }, "{label}" } }
    });
    #[cfg(feature = "rows")]
    let content = rsx! { for quote in rows.read().iter().copied() { QuoteRow { key: "{quote.index}", seed: quote, registry: registry.clone() } } };
    #[cfg(not(feature = "rows"))]
    let content = rsx! { for quote in rows.read().iter().copied() {
        div { key: "{quote.index}", role: "row", class: quote.class(),
            span { class: "symbol", "{quote.symbol()}" } span { class: "price", "{quote.price()}" }
            span { class: "quantity", "{quote.quantity()}" } span { class: "amount", "{quote.amount()}" }
        }
    } };
    rsx! { div { class: "toolbar", {buttons} } Total { value: total }
        div { class: "viewport", div { role: "table", class: "quotes", {content} } }
    }
}
fn main() {
    dioxus::launch(App);
}
#[wasm_bindgen]
pub fn stats() -> String {
    REGISTRY.with(|slot| {
        format!(
            "{{\"registry\":{}}}",
            slot.borrow().as_ref().unwrap().0.borrow().len()
        )
    })
}
