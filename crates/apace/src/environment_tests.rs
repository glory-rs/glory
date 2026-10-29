use super::focus_tests::{focus, selection, settle};
use super::tests::bounds_now;
use super::*;
use glory_render::style::{Display, FlexDirection, Length, Style};
use glory_render::{Change, Direction, Environment, LanguageTag, Patch, Semantics};
use glory_ui::{input, view};

fn metadata(direction: Direction, language: &str) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        environment: Some(Environment {
            direction,
            language: LanguageTag::new(language).unwrap(),
        }),
        ..Default::default()
    }))
}

#[test]
fn native_environment_changes_real_flex_direction_without_replacing_editors_or_preedit() {
    let mut app = WindowApp::new();
    let root = app.app.scope().cage(None);
    let first = app.app.scope().cage(None);
    let second = app.app.scope().cage(None);
    app.mount(
        view()
            .node_ref(root)
            .semantics(metadata(Direction::Ltr, "en-US"))
            .style(Style {
                display: Some(Display::Flex),
                flex_direction: Some(FlexDirection::Row),
                width: Some(Length::Px(400.)),
                height: Some(Length::Px(100.)),
                ..Default::default()
            })
            .fill((
                input()
                    .value("a🙂z")
                    .node_ref(first)
                    .style(Style::new().width(Length::Px(120.))),
                input()
                    .value("ثاني")
                    .node_ref(second)
                    .style(Style::new().width(Length::Px(120.))),
            )),
    )
    .unwrap();
    settle(&mut app);
    let first = first.get().unwrap();
    let second = second.get().unwrap();
    let left = bounds_now(&app, first);
    let right = bounds_now(&app, second);
    assert!(left.x < right.x, "LTR row has declaration order");
    focus(&mut app, first);
    app.handle_ui_event(UiEvent::Ime(ApaceImeEvent::Preedit(
        "中".into(),
        Some((3, 3)),
    )));
    settle(&mut app);
    let before_selection = selection(&mut app, first);
    let engine = app.renderer.0.borrow().dom.control(first).unwrap();
    let value = app.renderer.0.borrow().dom.text_value(first).unwrap();
    app.app
        .scope()
        .patch(
            root.get().unwrap(),
            Patch::Semantics(Change::Set(metadata(Direction::Rtl, "ar"))),
        )
        .unwrap();
    app.app.flush().unwrap();
    settle(&mut app);
    let left = bounds_now(&app, first);
    let right = bounds_now(&app, second);
    assert!(
        left.x > right.x,
        "RTL changes the actual engine flex layout"
    );
    assert_eq!(app.renderer.0.borrow().dom.control(first).unwrap(), engine);
    assert_eq!(selection(&mut app, first), before_selection);
    assert_eq!(
        app.renderer.0.borrow().dom.text_value(first).unwrap(),
        value
    );
    assert!(
        app.renderer
            .0
            .borrow()
            .dom
            .range(first)
            .unwrap()
            .editing
            .composing
    );
}
