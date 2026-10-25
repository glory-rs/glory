#[path = "../../model.rs"]
mod model;
use glory::prelude::*;
use model::{Market, Quote};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = scenarioCount)]
    fn count() -> usize;
}
type Registry = Rc<RefCell<HashMap<usize, Cage<Quote>>>>;
struct Demo {
    web: glory::web::WebApp,
    registry: Registry,
}
thread_local! { static APP: RefCell<Option<Demo>> = const { RefCell::new(None) }; }
#[derive(Clone)]
struct RowPlan {
    plan: Plan,
    row: PlanPart,
    symbol: PlanPart,
    price: PlanPart,
    quantity: PlanPart,
    amount: PlanPart,
}
impl RowPlan {
    fn new() -> Self {
        let plan = plan(view().role(Role::Row).class("quote").mark("row").fill((
            text().class("symbol").mark("symbol"),
            text().class("price").mark("price"),
            text().class("quantity").mark("quantity"),
            text().class("amount").mark("amount"),
        )))
        .unwrap();
        Self {
            row: plan.part("row").unwrap(),
            symbol: plan.part("symbol").unwrap(),
            price: plan.part("price").unwrap(),
            quantity: plan.part("quantity").unwrap(),
            amount: plan.part("amount").unwrap(),
            plan,
        }
    }
}
fn row(scope: &Scope, quote: Cage<Quote>, registry: Registry, declaration: &RowPlan) -> Result {
    let index = quote.with_untracked(|q| q.index);
    registry.borrow_mut().insert(index, quote);
    scope.owner().on_cleanup(move || {
        registry.borrow_mut().remove(&index);
    })?;
    let instance = scope.instantiate(&declaration.plan)?;
    instance.text(&declaration.symbol, quote.with_untracked(|q| q.symbol()))?;
    instance.text_fn(&declaration.price, move || quote.with(|q| q.price()))?;
    instance.text_fn(&declaration.quantity, move || quote.with(|q| q.quantity()))?;
    instance.text_fn(&declaration.amount, move || quote.with(|q| q.amount()))?;
    instance.class_fn(&declaration.row, move || quote.with(|q| q.class()))?;
    Ok(())
}
fn main() {
    let registry = Registry::default();
    let retained = registry.clone();
    let web = glory::web::launch_with(
        web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("main")
            .unwrap(),
        move |scope| {
            let rows = scope.cage(Vec::<Quote>::new());
            let total = scope.cage(0i64);
            let market = Rc::new(RefCell::new(Market::default()));
            let declaration = RowPlan::new();
            let mut buttons = Vec::new();
            for (class, label, kind) in [
                ("mount", "Open portfolio", 0),
                ("one", "One quote", 1),
                ("sparse", "1% quotes", 2),
                ("medium", "10% quotes", 3),
                ("dense", "All quotes", 4),
                ("burst", "10 market ticks", 5),
                ("select", "Next position", 6),
                ("clear", "Close portfolio", 7),
            ] {
                let market = market.clone();
                let registry = registry.clone();
                buttons.push(button().class(class).label(label).on(activate, move |_| {
                    let mut market = market.borrow_mut();
                    if kind == 0 {
                        if !market.quotes.is_empty() {
                            return;
                        }
                        let initial = market.mount(count());
                        rows.revise(|v| *v = initial);
                    } else if kind == 7 {
                        market.clear();
                        rows.revise(Vec::clear);
                    } else {
                        let n = market.quotes.len();
                        let edits = match kind {
                            1 => market.tick(1, 1),
                            2 => market.tick(n / 100, 1),
                            3 => market.tick(n / 10, 1),
                            4 => market.tick(n, 1),
                            5 => market.tick(n / 100, 10),
                            6 => market.select(),
                            _ => unreachable!(),
                        };
                        for quote in edits {
                            let target = registry.borrow()[&quote.index];
                            target.revise(|value| *value = quote);
                        }
                    }
                    total.revise(|value| *value = market.total);
                }));
            }
            group().fill((
                view().class("toolbar").fill(buttons),
                text()
                    .class("total")
                    .content_fn(move || model::money(total.get())),
                view()
                    .class("viewport")
                    .fill(view().role(Role::Table).class("quotes").fill(each(
                        rows,
                        |q| q.index,
                        move |quote, _| {
                            let registry = registry.clone();
                            let declaration = declaration.clone();
                            widget(move |scope| row(scope, quote, registry.clone(), &declaration))
                        },
                    ))),
            ))
        },
    )
    .unwrap();
    APP.with(|slot| {
        *slot.borrow_mut() = Some(Demo {
            web,
            registry: retained,
        })
    });
}
#[wasm_bindgen]
pub fn stats() -> String {
    APP.with(|slot| {
        let slot = slot.borrow();
        let demo = slot.as_ref().unwrap();
        let graph = demo.web.app().runtime_stats();
        format!(
            "{{\"registry\":{},\"nodes\":{},\"graph\":{},\"owners\":{}}}",
            demo.registry.borrow().len(),
            demo.web.app().stats().nodes,
            graph.nodes,
            graph.owners
        )
    })
}
