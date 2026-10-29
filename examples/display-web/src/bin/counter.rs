//! Ordinary application example: no row adapter, mount bookkeeping or cleanup table.
#[cfg(target_arch = "wasm32")]
mod browser {
    use g::display::prelude::*;
    use std::cell::RefCell;
    struct Item {
        id: usize,
        label: String,
    }
    #[g::component(g)]
    fn Counter(scope: &Scope, initial: usize) -> impl Widget {
        let count = scope.cage(initial);
        g::view! {<div class="counter"><span class="value">{count}</span><button class="increment" on:click={move||count.revise(|n|*n+=1)}>"+"</button></div>}
    }
    #[g::component(g)]
    fn Card(children: g::ui::Children) -> impl Widget {
        g::view! {<div class="card"><Content value={children}/></div>}
    }
    #[g::component(g)]
    fn Demo(scope: &Scope) -> impl Widget {
        let items = scope
            .keyed_list(Vec::<Item>::new(), |item| item.id)
            .unwrap();
        let next = scope.cage(1usize);
        let visible = scope.cage(true);
        let selected = scope.cage(None);
        g::view! {
            <div>
                <button class="toggle" on:click={move||visible.revise(|v|*v = !*v)}>"Toggle counter"</button>
                <Show when={move||visible.get()}><Card><Counter initial={0}/></Card></Show>
                <button class="add" on:click={move||{let id=next.get();next.revise(|n|*n+=1);items.extend(vec![Item{id,label:format!("Item {id}")}]).unwrap();}}>"Add item"</button>
                <button class="clear" on:click={move||items.clear().unwrap()}>"Clear"</button>
                <span class="total">{move||items.with_order(|rows|rows.len())}</span>
                <div class="rows"><Each each={items} children={move|row|{
                    let id=*row.key();let label=g::field!(row.label);
                    g::view!{
                        <div class="item" class:selected={move||selected.get()==Some(id)}>
                            <button class="select" on:click={move||selected.revise(|v|*v=Some(id))}>{label}</button>
                            <button class="edit" on:click={move||items.update_key(&id,|item|item.label.push('!')).unwrap()}>"Edit"</button>
                            <button class="remove" on:click={move||{items.remove_key(&id).unwrap();}}>"Remove"</button>
                        </div>
                    }
                }}/></div>
            </div>
        }
    }
    thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};}
    pub fn main() {
        APP.with(|slot| *slot.borrow_mut() = Some(App::launch("main", Demo::default()).unwrap()));
    }
}
fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::main();
}
