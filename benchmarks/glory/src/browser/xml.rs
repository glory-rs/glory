use crate::model::Model;
use glory::display::*;
pub fn view(model: Model) -> impl IntoView {
    let row_model = model.clone();
    view! {
        <>
            <button type="button" class="bench-run" on:click={model.handler(0)}>"Create 1,000 rows"</button>
            <button type="button" class="bench-runlots" on:click={model.handler(1)}>"Create 10,000 rows"</button>
            <button type="button" class="bench-add" on:click={model.handler(2)}>"Append 1,000 rows"</button>
            <button type="button" class="bench-update" on:click={model.handler(3)}>"Update every 10th row"</button>
            <button type="button" class="bench-clear" on:click={model.handler(4)}>"Clear"</button>
            <button type="button" class="bench-swaprows" on:click={model.handler(5)}>"Swap rows"</button>
            <div role="table" class="bench-table"><div class="bench-body">
                <Each each={model.rows} children={move |row| {
                    let id=*row.key();let label=field!(row.label);
                    view! {
                        <div role="row" class="bench-row" class:danger={computed(row_model.is_selected(id))}>
                            <div role="cell" class="bench-cell col-md-1"><span>{id}</span></div>
                            <div role="cell" class="bench-cell col-md-4"><button type="button" class="lbl" on:click={row_model.select(id)}>{label}</button></div>
                            <div role="cell" class="bench-cell col-md-1"><button type="button" class="remove" on:click={row_model.remove(id)}>"x"</button></div>
                            <div role="cell" class="bench-cell col-md-6"/>
                        </div>
                    }
                }}/>
            </div></div>
        </>
    }
}
