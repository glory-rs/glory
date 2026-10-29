use super::*;
use glory_render::{Semantics, TranslationKind, TranslationMotion, TranslationOffset};
#[test]
fn translated_inert_visual_copies_are_absent_while_static_text_is_announced_once() {
    let app = WindowApp::new();
    app.mount(
        view().fill((
            text().content("Balance 123"),
            view()
                .semantics(Some(Box::new(Semantics {
                    inert: true,
                    translation: Some(TranslationMotion {
                        kind: TranslationKind::To {
                            target: TranslationOffset::new(0., -36.),
                            duration_millis: 250,
                        },
                        paused: false,
                    }),
                    ..Default::default()
                })))
                .fill((text().content("Balance 123"), text().content("Balance 123"))),
        )),
    )
    .unwrap();
    let tree = Consumer::new(build(&app, true), true);
    assert_eq!(
        nodes(&tree)
            .into_iter()
            .filter(|node| node.data().value() == Some("Balance 123"))
            .count(),
        1
    );
    let root = app.app.tree().roots()[0];
    let visual = app.app.tree().node(root).unwrap().children[1];
    assert!(
        !app.accessibility
            .borrow()
            .ids
            .contains_key(&Key::Main(visual))
    );
    for child in &app.app.tree().node(visual).unwrap().children {
        assert!(
            !app.accessibility
                .borrow()
                .ids
                .contains_key(&Key::Main(*child))
        );
    }
}
