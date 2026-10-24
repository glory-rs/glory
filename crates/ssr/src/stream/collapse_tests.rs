use super::*;
use glory_render::{CollapseConfig, EventKind, Semantics};
#[test]
fn finite_collapse_config_revision_and_classic_end_use_the_current_html_format() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "collapse",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view().class("stream-collapse-container").fill((
                view()
                    .class("stream-collapse-panel")
                    .semantics(ready.map(|state| {
                        let open = matches!(state, ResourceState::Ready { .. });
                        Some(Box::new(Semantics {
                            collapse: Some(CollapseConfig {
                                open,
                                duration_millis: 240,
                                revision: u64::from(open),
                            }),
                            hidden: !open,
                            inert: !open,
                            ..Default::default()
                        }))
                    }))
                    .on(EventKind::CollapseComplete, |_| {})
                    .fill(
                        view()
                            .style(
                                glory_render::style::Style::new()
                                    .height(glory_render::style::Length::Px(96.)),
                            )
                            .fill(
                                glory_ui::input()
                                    .class("stream-collapse-editor")
                                    .name("Streaming draft")
                                    .value("stream draft"),
                            ),
                    ),
                glory_ui::button().label("Close streamed collapse"),
                glory_ui::button().label("Open streamed collapse"),
                text()
                    .class("stream-collapse-completed")
                    .content("Completed: 0"),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-collapse=\"closed\""));
    assert!(
        shell
            .body()
            .contains("data-gly-collapse-duration=\"240ms\"")
    );
    assert!(shell.body().contains("data-gly-collapse-revision=\"0\""));
    assert!(!shell.body().contains(" hidden=\"\""));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(
        patch.body().contains("data-gly-collapse-revision") && patch.body().contains("aria-hidden")
    );
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_COLLAPSE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "collapse-stream-root".into(),
            data_id: "collapse-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
