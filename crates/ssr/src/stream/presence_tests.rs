use super::*;
use glory_render::{PresenceConfig, PresenceEffect, Semantics};
#[test]
fn presence_stream_patches_current_configuration_and_rekeys_without_replacing_content() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "presence",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|s| matches!(s, ResourceState::Ready { .. }));
            view()
                .semantics(scope.bond(move || {
                    let open = ready.get();
                    Some(Box::new(Semantics {
                        hidden: !open,
                        inert: !open,
                        presence: Some(Box::new(PresenceConfig {
                            open,
                            effect: PresenceEffect::Scale,
                            duration_millis: 200,
                            revision: u64::from(open),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }))
                }))
                .fill(
                    glory_ui::input()
                        .name("Retained stream draft")
                        .value("draft"),
                )
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-presence=\"closed\""));
    assert!(shell.body().contains("data-gly-presence-effect=\"scale\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for marker in ["data-gly-presence", "open", "data-gly-presence-revision"] {
        assert!(patch.body().contains(marker), "{marker}");
    }
    assert!(!patch.body().contains("\"kind\":\"create\""));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_PRESENCE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "presence-root".into(),
            data_id: "presence-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
