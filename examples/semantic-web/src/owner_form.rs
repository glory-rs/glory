use glory::prelude::*;
use std::{cell::Cell, rc::Rc};

pub type Values = (Cage<usize>, Bond<usize>);
pub struct State {
    pub model: Cage<(usize, Vec<u8>)>,
    pub last: Rc<Cell<Option<Values>>>,
}
pub fn ui(scope: &glory::ui::Scope) -> (impl Widget + use<>, State) {
    let model = scope.cage((0usize, vec![9u8; 1024 * 1024]));
    let shown = scope.cage(true);
    let last = Rc::new(Cell::new(None));
    let captures = last.clone();
    let widget = view().fill((
        text()
            .class("owner-count")
            .content(model.map(|value| value.0.to_string())),
        button()
            .class("owner-toggle")
            .fill(text().content("Toggle"))
            .on(glory::render::EventKind::Activate, move |_| {
                shown.revise(|shown| *shown = !*shown)
            }),
        when(shown, move || {
            captures.set(None);
            let out = captures.clone();
            let mut derived = None;
            button()
                .class("owner-edit")
                .fill(text().content("Edit"))
                .on(glory::render::EventKind::Activate, move |_| {
                    let field = model.field("count", |value| &value.0, |value| &mut value.0);
                    let value = *derived.get_or_insert_with(|| field.map(|value| *value));
                    out.set(Some((field, value)));
                    field.revise(|value| *value += 1);
                })
        }),
    ));
    (widget, State { model, last })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_retires_the_source_model_without_cloning_the_payload() {
        let mut model = None;
        let snapshot = glory::ssr::snapshot_with(|scope| {
            let (widget, state) = ui(scope);
            model = Some(state.model);
            widget
        })
        .unwrap();
        assert!(snapshot.html.contains("owner-edit"));
        assert!(snapshot.html.contains("Toggle"));
        assert!(!model.unwrap().is_alive());
    }
}
