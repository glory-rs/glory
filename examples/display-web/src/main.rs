#[cfg(target_arch = "wasm32")]
mod browser {
    use g::Cage;
    use g::display::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::prelude::*;

    #[cfg(feature = "hand")]
    const COUNT: Field = Field::new("count");
    #[cfg(feature = "hand")]
    const DOUBLE: Field = Field::new("double");
    #[cfg(feature = "hand")]
    const SHARED: Field = Field::new("shared");
    #[cfg(feature = "hand")]
    const CLASS: Field = Field::new("class");
    #[cfg(feature = "hand")]
    const ONCE: Field = Field::new("once");
    #[cfg(feature = "hand")]
    const INC: Click = Click::new("increment");
    #[cfg(feature = "hand")]
    const TOGGLE: Click = Click::new("toggle");
    #[cfg(feature = "hand")]
    const ALT: Click = Click::new("alt");
    #[cfg(feature = "hand")]
    const CHILD: Outlet = Outlet::new("child");
    const ROWS: Outlet = Outlet::new("rows");
    const LABEL: Field = Field::new("label");
    const PICK: Click = Click::new("pick");

    #[cfg(feature = "hand")]
    static COUNTER: Template = Template::new(&[div(
        &[class("counter")],
        &[
            span(&[class("value")], &[field(COUNT)]),
            span(&[class("double")], &[field(DOUBLE)]),
            span(&[class("shared")], &[field(SHARED)]),
            button(
                &[attr("type", "button"), class("increment"), on_click(INC)],
                &[text("+")],
            ),
        ],
    )]);
    #[g::component(g)]
    fn Counter(scope: &Scope, initial: i32, shared: Cage<i32>) -> impl Widget {
        let count = scope.cage(initial);
        // This implicit map must belong to the component, not its source.
        let shared = shared.map(|value| format!("shared {value}"));
        #[cfg(feature = "hand")]
        {
            view(&COUNTER)
                .bind(COUNT, count)
                .watch(DOUBLE, move || count.get() * 2)
                .bind(SHARED, shared)
                .on_click(INC, move || count.revise(|n| *n += 1))
        }
        #[cfg(not(feature = "hand"))]
        {
            g::view! {
                <div class="counter">
                    <span class="value">{count}</span>
                    <span class="double">{move || count.get()*2}</span>
                    <span class="shared">{shared}</span>
                    <button type="button" class="increment" on:click={move || count.revise(|n|*n+=1)}>"+"</button>
                </div>
            }
        }
    }
    #[cfg(feature = "hand")]
    static ROOT: Template = Template::new(&[div(
        &[bind_attr("class", CLASS)],
        &[
            span(&[class("value")], &[field(COUNT)]),
            span(&[class("computed")], &[field(DOUBLE)]),
            span(&[class("once")], &[field(ONCE)]),
            button(
                &[attr("type", "button"), class("increment"), on_click(INC)],
                &[text("+")],
            ),
            button(
                &[attr("type", "button"), class("toggle"), on_click(TOGGLE)],
                &[text("toggle")],
            ),
            button(
                &[attr("type", "button"), class("alt"), on_click(ALT)],
                &[text("alt")],
            ),
            outlet(CHILD),
            div(&[class("rows"), children(ROWS)], &[]),
            span(&[class("escaped")], &[text("<div>\"汉字🙂&")]),
        ],
    )]);
    static ROW: Template = Template::new(&[button(
        &[attr("type", "button"), class("row"), on_click(PICK)],
        &[field(LABEL)],
    )]);
    struct Demo {
        app: App,
        scope: Rc<Scope>,
        count: Cage<i32>,
        css: Cage<String>,
        reads: Rc<Cell<usize>>,
        rows: Children,
        owned: Vec<(Scope, Mounted)>,
    }
    thread_local! {static APP:RefCell<Option<Demo>>=const{RefCell::new(None)};}
    pub fn main() {
        let root = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("main")
            .unwrap();
        let app = App::new(root).unwrap();
        let scope = app.scope();
        let count = scope.cage(0);
        let alt = scope.cage(100);
        let selected = scope.cage(true);
        let css = scope.cage("demo".to_owned());
        let reads = Rc::new(Cell::new(0));
        let reads_in_view = reads.clone();
        let read = move || {
            reads_in_view.set(reads_in_view.get() + 1);
            if selected.get() {
                count.get()
            } else {
                alt.get()
            }
        };
        #[cfg(feature = "hand")]
        let config = view(&ROOT)
            .bind(CLASS, css)
            .bind(COUNT, count)
            .watch(DOUBLE, read)
            .bind(ONCE, count.get())
            .on_click(INC, move || count.revise(|n| *n += 1))
            .on_click(TOGGLE, move || selected.revise(|n| *n = !*n))
            .on_click(ALT, move || alt.revise(|n| *n += 1))
            .child(
                CHILD,
                lazy(move || Counter::default().initial(10).shared(count)),
            );
        #[cfg(not(feature = "hand"))]
        let config = g::view! {
            <div class={css}>
                <span class="value">{count}</span>
                <span class="computed">{computed(read)}</span>
                <span class="once">{count.get()}</span>
                <button type="button" class="increment" on:click={move || count.revise(|n|*n+=1)}>"+"</button>
                <button type="button" class="toggle" on:click={move || selected.revise(|n|*n = !*n)}>"toggle"</button>
                <button type="button" class="alt" on:click={move || alt.revise(|n|*n+=1)}>"alt"</button>
                <Counter initial={10} shared={count}/>
                <div class="rows" children={ROWS}/>
                <span class="escaped">"<div>\"汉字🙂&"</span>
            </div>
        };
        let mounted = app.render(config).unwrap();
        let rows = mounted.children(ROWS).unwrap();
        APP.with(|slot| {
            *slot.borrow_mut() = Some(Demo {
                app,
                scope,
                count,
                css,
                reads,
                rows,
                owned: Vec::new(),
            })
        });
    }
    #[wasm_bindgen]
    pub fn add_rows(count: usize) {
        APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            let demo = slot.as_mut().unwrap();
            for index in 0..count {
                let scope = demo.scope.child();
                let label = scope.cage(format!("row {index} 汉🙂"));
                let selected = demo.count;
                let mounted = view(&ROW)
                    .bind(LABEL, label)
                    .on_click(PICK, move || selected.revise(|n| *n += 100))
                    .mount(&demo.app, &scope)
                    .unwrap();
                mounted.place(Some(&demo.rows), None).unwrap();
                demo.owned.push((scope, mounted));
            }
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn computed_calls() -> usize {
        APP.with(|slot| slot.borrow().as_ref().unwrap().reads.get())
    }
    #[wasm_bindgen]
    pub fn set_class(value: String) {
        APP.with(|slot| {
            let slot = slot.borrow();
            let demo = slot.as_ref().unwrap();
            demo.css.revise(|v| *v = value);
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn add_components(count: usize) {
        APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            let demo = slot.as_mut().unwrap();
            for index in 0..count {
                let scope = demo.scope.child();
                let mounted = g::web::view::mount(
                    Counter::default().initial(index as i32).shared(demo.count),
                    &demo.app,
                    &scope,
                )
                .unwrap();
                mounted.place(Some(&demo.rows), None).unwrap();
                demo.owned.push((scope, mounted));
            }
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn clear_rows() {
        APP.with(|slot| {
            let mut slot = slot.borrow_mut();
            let demo = slot.as_mut().unwrap();
            demo.owned.clear();
            demo.app.flush().unwrap();
        });
    }
    #[wasm_bindgen]
    pub fn stats() -> String {
        APP.with(|slot| {
            let slot = slot.borrow();
            let demo = slot.as_ref().unwrap();
            let d = demo.app.stats();
            let r = demo.app.runtime().stats();
            format!(
                "{{\"instances\":{},\"targets\":{},\"records\":{},\"graph\":{},\"owners\":{}}}",
                d.instances, d.targets, d.records, r.nodes, r.owners
            )
        })
    }
    #[wasm_bindgen]
    pub fn close() {
        APP.with(|slot| {
            if let Some(demo) = slot.borrow_mut().take() {
                demo.app.close().unwrap();
            }
        });
    }
}
#[cfg(target_arch = "wasm32")]
mod structural;
#[cfg(target_arch = "wasm32")]
mod unified;
fn main() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into())
    }));
    #[cfg(target_arch = "wasm32")]
    browser::main();
    #[cfg(target_arch = "wasm32")]
    structural::main();
    #[cfg(target_arch = "wasm32")]
    unified::main();
}
