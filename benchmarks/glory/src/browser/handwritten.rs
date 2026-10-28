use super::super::model::Model;
use glory::display::*;
const BODY: Outlet = Outlet::new("rows");
#[path = "declarations.rs"]
mod declarations;
pub fn view(model: Model) -> impl IntoView {
    let rows = model.rows;
    let row_model = model.clone();
    glory::display::view(&declarations::PANEL)
        .on_click(declarations::RUN, model.handler(0))
        .on_click(declarations::LOTS, model.handler(1))
        .on_click(declarations::ADD, model.handler(2))
        .on_click(declarations::UPDATE, model.handler(3))
        .on_click(declarations::CLEAR, model.handler(4))
        .on_click(declarations::SWAP, model.handler(5))
        .child(
            BODY,
            each(rows, move |row| {
                let id = *row.key();
                glory::display::view(&declarations::ROW)
                    .class_when(declarations::CLASS, computed(row_model.is_selected(id)))
                    .bind(declarations::ID, id)
                    .bind(declarations::LABEL, field!(row.label))
                    .on_click(declarations::SELECT, row_model.select(id))
                    .on_click(declarations::REMOVE, row_model.remove(id))
            }),
        )
}
