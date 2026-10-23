use glory::prelude::*;
use glory::render::Event;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Row {
    pub title: String,
    pub draft: String,
    pub payload: Vec<u8>,
}
pub fn row(key: u64) -> Row {
    Row {
        title: format!("Key <{key}>"),
        draft: format!("draft{key}🙂"),
        payload: vec![9; 1024 * 1024],
    }
}
pub struct State {
    pub rows: Cage<HashMap<u64, Row>>,
    pub order: Cage<Vec<usize>>,
    pub title: Cage<String>,
    pub runs: Rc<[Cell<usize>; 4]>,
    pub positions: Rc<Cell<usize>>,
}
pub fn ui(scope: &glory::ui::Scope) -> (impl Widget + use<>, State) {
    let rows = scope.cage(HashMap::from([(7u64, row(7)), (9, row(9))]));
    let order = scope.cage(vec![7usize, 9]);
    let first = order.item(0);
    let positions = Rc::new(Cell::new(0));
    let position_calls = positions.clone();
    let a = rows.entry(7);
    let b = rows.entry(9);
    let at = a.field("title", |r| &r.title, |r| &mut r.title);
    let ad = a.field("draft", |r| &r.draft, |r| &mut r.draft);
    let bt = b.field("title", |r| &r.title, |r| &mut r.title);
    let bd = b.field("draft", |r| &r.draft, |r| &mut r.draft);
    let runs = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0)]);
    let source = [at, ad, bt, bd];
    let values: [Bond<String>; 4] = std::array::from_fn(|index| {
        let runs = runs.clone();
        let value = source[index];
        scope
            .bond(move || {
                runs[index].set(runs[index].get() + 1);
                value.try_get().unwrap_or_else(|_| {
                    if index % 2 == 0 {
                        "missing".into()
                    } else {
                        String::new()
                    }
                })
            })
            .with_partial_eq()
    });
    let widget = view().class("entries-frame").fill((
        text().class("entry-title7").content(values[0]),
        input()
            .class("entry-input7")
            .value(values[1])
            .enabled(
                scope
                    .bond(move || a.try_with(|_| ()).is_ok())
                    .with_partial_eq(),
            )
            .on(value_changed, move |e| {
                if let Event::ValueChanged { value, .. } = e {
                    let _ = ad.try_revise(|v| *v = value);
                }
            }),
        text().class("entry-title9").content(values[2]),
        input()
            .class("entry-input9")
            .value(values[3])
            .enabled(
                scope
                    .bond(move || b.try_with(|_| ()).is_ok())
                    .with_partial_eq(),
            )
            .on(value_changed, move |e| {
                if let Event::ValueChanged { value, .. } = e {
                    let _ = bd.try_revise(|v| *v = value);
                }
            }),
        text().class("entry-position").content(first.map(move |v| {
            position_calls.set(position_calls.get() + 1);
            v.to_string()
        })),
    ));
    (
        widget,
        State {
            rows,
            order,
            title: at,
            runs,
            positions,
        },
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn keyed_non_clone_rows_serialize_and_release_with_the_request() {
        let mut state = None;
        let snapshot = glory::ssr::snapshot_with(|scope| {
            let (widget, data) = super::ui(scope);
            state = Some(data);
            widget
        })
        .unwrap();
        let state = state.unwrap();
        assert!(snapshot.html.contains("Key &lt;7&gt;"));
        assert!(snapshot.html.contains("value=\"draft9🙂\""));
        assert_eq!(state.runs.each_ref().map(|n| n.get()), [1; 4]);
        assert!(!state.rows.is_alive() && !state.title.is_alive() && !state.order.is_alive());
    }
}
