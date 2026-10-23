use glory::prelude::*;
use glory::render::{Change, Event, EventKind, Patch};

pub struct State {
    pub rows: Cage<Vec<usize>>,
    pub value: Cage<String>,
    pub checked: Cage<bool>,
    pub caption: Cage<String>,
}
pub fn ui(scope: &Scope) -> (impl Widget + use<>, State) {
    let rows = scope.cage(vec![0usize, 1]);
    let value = scope.cage("a🙂b".to_owned());
    let checked = scope.cage(false);
    let caption = scope.cage("caption".to_owned());
    let declaration = plan(view().class("plan-row").mark("row").fill((
        text().mark("caption"),
        input().class("plan-editor").mark("editor"),
        check().class("plan-check").label("check").mark("check"),
    )))
    .unwrap();
    let root = declaration.part("row").unwrap();
    let text = declaration.part("caption").unwrap();
    let editor = declaration.part("editor").unwrap();
    let check = declaration.part("check").unwrap();
    let fallback = plan(group().fill(text_builder())).unwrap();
    let widget = view().class("plan-form").fill((
        each(
            rows,
            |id| *id,
            move |id, _| {
                let (declaration, root, text, editor, check) = (
                    declaration.clone(),
                    root.clone(),
                    text.clone(),
                    editor.clone(),
                    check.clone(),
                );
                component(move |scope| {
                    let id = id.get_untracked();
                    let instance = scope.instantiate(&declaration)?;
                    instance.class(&root, format!("plan-row plan-{id}"))?;
                    instance.text(&text, caption)?;
                    if id == 0 {
                        instance.bind(&editor, value, |value| Patch::Value {
                            value: Change::Set(value),
                            revision: 0,
                        })?;
                        instance
                            .bind(&check, checked, |value| Patch::Checked(Change::Set(value)))?;
                        instance.on(&editor, EventKind::ValueChanged, move |event| {
                            if let Event::ValueChanged { value: next, .. } = event {
                                value.revise(|v| *v = next);
                            }
                        })?;
                        instance.on(&check, EventKind::CheckedChanged, move |event| {
                            if let Event::CheckedChanged(next) = event {
                                checked.revise(|v| *v = next);
                            }
                        })?;
                    } else {
                        instance.bind(&editor, format!("other {id}"), |value| Patch::Value {
                            value: Change::Set(value),
                            revision: 0,
                        })?;
                    }
                    Ok(())
                })
            },
        ),
        component(move |scope| {
            scope.instantiate(&fallback)?;
            Ok(())
        }),
    ));
    (
        widget,
        State {
            rows,
            value,
            checked,
            caption,
        },
    )
}
fn text_builder() -> glory::ui::Text {
    text().class("plan-fallback").content("group")
}

#[cfg(test)]
mod tests {
    #[test]
    fn plans_expand_to_snapshot_and_release_all_sources() {
        let mut source = None;
        let snapshot = glory::ssr::snapshot_with(|scope| {
            let (widget, state) = super::ui(scope);
            source = Some(state.value);
            widget
        })
        .unwrap();
        assert_eq!(snapshot.html.matches("plan-editor").count(), 2);
        assert!(snapshot.html.contains("other 1"));
        assert!(snapshot.html.contains("a🙂b"));
        assert!(snapshot.html.contains("plan-fallback"));
        assert!(!source.unwrap().is_alive());
    }
}
