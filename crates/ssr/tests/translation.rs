use glory_render::{Semantics, TranslationKind, TranslationMotion, TranslationOffset};
use glory_ui::{text, view};
#[test]
fn translation_ssr_is_static_without_css_attr_or_application_animation_work() {
    let snapshot = glory_ssr::snapshot(
        view()
            .semantics(Some(Box::new(Semantics {
                inert: true,
                translation: Some(TranslationMotion {
                    kind: TranslationKind::To {
                        target: TranslationOffset::new(12., -24.),
                        duration_millis: 300,
                    },
                    paused: false,
                }),
                ..Default::default()
            })))
            .fill(text().content("visual copy")),
    )
    .unwrap();
    assert!(
        snapshot.html.contains("transform:translate(12px,-24px);")
            && snapshot.html.contains("data-gly-translation=\"to\"")
            && snapshot.html.contains("inert=\"\"")
    );
    assert!(
        snapshot
            .html
            .contains(&format!("gly:v:{}", glory_html::FORMAT_VERSION))
    );
}
