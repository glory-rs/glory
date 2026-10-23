use glory::prelude::*;
use glory::render::Event;
use std::cell::Cell;
use std::rc::Rc;

pub struct Model {
    pub title: String,
    pub draft: String,
    pub background: usize,
    pub payload: Vec<u8>,
}
pub struct State {
    pub model: Cage<Model>,
    pub title: Cage<String>,
    pub draft: Cage<String>,
    pub background: Cage<usize>,
    pub runs: Rc<[Cell<usize>; 3]>,
}
pub fn ui(scope: &glory::ui::Scope) -> (impl Widget + use<>, State) {
    let model = scope.cage(Model {
        title: "Field <title>".into(),
        draft: "initial🙂".into(),
        background: 0,
        payload: vec![9; 1024 * 1024],
    });
    let title = model.field("title", |m| &m.title, |m| &mut m.title);
    let draft = model.field("draft", |m| &m.draft, |m| &mut m.draft);
    let background = model.field("background", |m| &m.background, |m| &mut m.background);
    let runs = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0)]);
    let a = runs.clone();
    let b = runs.clone();
    let c = runs.clone();
    let widget = view().class("fields-frame").fill((
        text().class("fields-title").content(title.map(move |v| {
            a[0].set(a[0].get() + 1);
            v.clone()
        })),
        input()
            .class("fields-editor")
            .value(draft.map(move |v| {
                b[1].set(b[1].get() + 1);
                v.clone()
            }))
            .on(value_changed, move |event| {
                if let Event::ValueChanged { value, .. } = event {
                    draft.revise(|v| *v = value);
                }
            }),
        text()
            .class("fields-background")
            .content(background.map(move |v| {
                c[2].set(c[2].get() + 1);
                v.to_string()
            })),
    ));
    (
        widget,
        State {
            model,
            title,
            draft,
            background,
            runs,
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn field_based_ui_serializes_and_releases_its_model_with_the_snapshot() {
        let mut state = None;
        let snapshot = glory::ssr::snapshot_with(|scope| {
            let (widget, data) = super::ui(scope);
            state = Some(data);
            widget
        })
        .unwrap();
        let state = state.unwrap();
        assert!(snapshot.html.contains("Field &lt;title&gt;"));
        assert!(snapshot.html.contains("value=\"initial🙂\""));
        assert_eq!(state.runs.each_ref().map(|n| n.get()), [1, 1, 1]);
        assert!(!state.model.is_alive());
        assert!(!state.draft.is_alive());
    }
}
