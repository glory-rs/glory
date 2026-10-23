//! Pure semantic builders, full keyed each, and row-owned reactive labels.
#[cfg(target_arch = "wasm32")]
#[path = "../../data.rs"]
mod data;
#[cfg(target_arch = "wasm32")]
mod browser {
    use super::data::Generator;
    use glory::prelude::*;
    use std::{cell::RefCell, collections::HashMap, rc::Rc};
    use wasm_bindgen::prelude::*;
    #[derive(Clone)]
    struct Row {
        id: usize,
        initial: Rc<str>,
    }
    #[derive(Clone)]
    struct Model {
        rows: Cage<Vec<Row>>,
        selected: Cage<Option<usize>>,
        generator: Rc<RefCell<Generator>>,
        labels: Rc<RefCell<HashMap<usize, Cage<String>>>>,
    }
    impl Model {
        fn make(&self, count: usize) -> Vec<Row> {
            self.generator
                .borrow_mut()
                .rows(count)
                .into_iter()
                .map(|row| Row {
                    id: row.id,
                    initial: row.label.into(),
                })
                .collect()
        }
    }
    struct Demo {
        web: glory::web::WebApp,
        model: Model,
    }
    thread_local! { static APP: RefCell<Option<Demo>> = const { RefCell::new(None) }; }
    fn row(scope: &Scope, seed: Cage<Row>, model: Model) -> Result {
        let seed = seed.get_untracked();
        let id = seed.id;
        let label = scope.cage(seed.initial.to_string());
        model.labels.borrow_mut().insert(id, label);
        let labels = model.labels.clone();
        scope.owner().on_cleanup(move || {
            labels.borrow_mut().remove(&id);
        })?;
        let selected = model.selected;
        let rows = model.rows;
        view()
            .role(Role::Row)
            .class(selected.map(move |value| {
                if *value == Some(id) {
                    "bench-row danger".to_owned()
                } else {
                    "bench-row".to_owned()
                }
            }))
            .fill((
                view()
                    .role(Role::Cell)
                    .class("bench-cell col-md-1")
                    .fill(text().content(id.to_string())),
                view().role(Role::Cell).class("bench-cell col-md-4").fill(
                    button().class("lbl").label(label).on(activate, move |_| {
                        selected.revise(|value| *value = Some(id));
                    }),
                ),
                view().role(Role::Cell).class("bench-cell col-md-1").fill(
                    button().class("remove").label("x").on(activate, move |_| {
                        rows.revise(|rows| rows.retain(|row| row.id != id));
                    }),
                ),
                view().role(Role::Cell).class("bench-cell col-md-6"),
            ))
            .build(scope)
    }
    fn ui(model: Model) -> impl Widget {
        let (run, lots, add, update, clear, swap) = (
            model.clone(),
            model.clone(),
            model.clone(),
            model.clone(),
            model.clone(),
            model.clone(),
        );
        let rows = model.rows;
        group().fill((
            button()
                .class("bench-run")
                .label("Create 1,000 rows")
                .on(activate, move |_| {
                    let next = run.make(1000);
                    run.rows.revise(|rows| *rows = next);
                }),
            button()
                .class("bench-runlots")
                .label("Create 10,000 rows")
                .on(activate, move |_| {
                    let next = lots.make(10000);
                    lots.rows.revise(|rows| *rows = next);
                }),
            button()
                .class("bench-add")
                .label("Append 1,000 rows")
                .on(activate, move |_| {
                    let next = add.make(1000);
                    add.rows.revise(|rows| rows.extend(next));
                }),
            button()
                .class("bench-update")
                .label("Update every 10th row")
                .on(activate, move |_| {
                    update.rows.with_untracked(|rows| {
                        for row in rows.iter().step_by(10) {
                            let label = update.labels.borrow()[&row.id];
                            label.revise(|value| value.push_str(" !!!"));
                        }
                    });
                }),
            button()
                .class("bench-clear")
                .label("Clear")
                .on(activate, move |_| {
                    clear.rows.revise(Vec::clear);
                    clear.selected.revise(|value| *value = None);
                }),
            button()
                .class("bench-swaprows")
                .label("Swap rows")
                .on(activate, move |_| {
                    swap.rows.revise(|rows| {
                        if rows.len() > 998 {
                            rows.swap(1, 998);
                        }
                    });
                }),
            view()
                .role(Role::Table)
                .class("bench-table")
                .fill(view().class("bench-body").fill(each(
                    rows,
                    |row| row.id,
                    move |seed, _| {
                        let model = model.clone();
                        component(move |scope| row(scope, seed, model.clone()))
                    },
                ))),
        ))
    }
    pub fn main() {
        let document = web_sys::window().unwrap().document().unwrap();
        let root = document.get_element_by_id("main").unwrap();
        let mut state = None;
        let web = glory::web::launch_with(root, |scope| {
            let model = Model {
                rows: scope.cage(Vec::new()),
                selected: scope.cage(None),
                generator: Rc::new(RefCell::new(Generator::default())),
                labels: Rc::new(RefCell::new(HashMap::new())),
            };
            state = Some(model.clone());
            ui(model)
        })
        .unwrap();
        APP.with(|slot| {
            *slot.borrow_mut() = Some(Demo {
                web,
                model: state.unwrap(),
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
                "{{\"rows\":{},\"labels\":{},\"nodes\":{},\"graph\":{},\"owners\":{}}}",
                demo.model.rows.with_untracked(Vec::len),
                demo.model.labels.borrow().len(),
                demo.web.app().stats().nodes,
                graph.nodes,
                graph.owners
            )
        })
    }
}
fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::main();
}
