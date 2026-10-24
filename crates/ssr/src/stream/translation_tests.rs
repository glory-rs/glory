use super::*;
use glory_render::{Semantics, TranslationKind, TranslationMotion, TranslationOffset};
#[test]
fn finite_translation_survives_rust_stream_patch_and_current_format_handoff() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "translation",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view()
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(80.))
                        .height(glory_render::style::Length::Px(40.))
                        .overflow_x(glory_render::style::Overflow::Hidden),
                )
                .fill(
                    view()
                        .class("stream-translation")
                        .semantics(ready.map(|state| {
                            Some(Box::new(Semantics {
                                inert: true,
                                translation: Some(TranslationMotion {
                                    kind: TranslationKind::Loop {
                                        from: TranslationOffset::ZERO,
                                        to: TranslationOffset::new(80., 0.),
                                        period_millis: if matches!(
                                            state,
                                            ResourceState::Ready { .. }
                                        ) {
                                            800
                                        } else {
                                            1600
                                        },
                                    },
                                    paused: false,
                                }),
                                ..Default::default()
                            }))
                        }))
                        .style(glory_render::style::Style {
                            background: Some(glory_render::style::Color(255, 0, 0, 255)),
                            ..glory_render::style::Style::new()
                                .width(glory_render::style::Length::Px(20.))
                                .height(glory_render::style::Length::Px(20.))
                        }),
                )
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(
        shell.body().contains("data-gly-translation=\"loop\"") && shell.body().contains("1600")
    );
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("data-gly-translation-duration") && patch.body().contains("800"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_TRANSLATION_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "translation-stream-root".into(),
            data_id: "translation-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
