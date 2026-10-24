use super::*;
use glory_render::{FocusMode, FocusScopeConfig, SemanticId, Semantics};

#[test]
fn focus_scope_policy_fields_and_classic_start_end_share_the_current_format() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "focus-scope",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            view()
                .focus(FocusMode::Programmatic)
                .semantics(scope.bond(move || {
                    let ready = ready.get();
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-focus-scope").unwrap()),
                        focus_scope: Some(FocusScopeConfig {
                            active: ready,
                            loop_tab: ready,
                            contain_focus: ready,
                            auto_focus: ready,
                            restore_focus: ready,
                            initial_focus: ready
                                .then(|| SemanticId::new("stream-initial").unwrap()),
                            return_focus: ready.then(|| SemanticId::new("stream-return").unwrap()),
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(glory_ui::input().name("Stream focus editor"))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-focus-scope=\"false\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    for name in [
        "scope", "loop", "contain", "auto", "restore", "initial", "return",
    ] {
        assert!(
            patch.body().contains(&format!("data-gly-focus-{name}")),
            "{name}"
        );
    }
    let end = frame(&mut stream);
    let bootstrap = Bootstrap {
        root_id: "focus-stream-root".into(),
        data_id: "focus-stream-data".into(),
        script_url: "glory-stream.js".into(),
        nonce: None,
    };
    let shell_html = shell.to_html(&bootstrap).unwrap();
    assert!(shell_html.contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION)));
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_FOCUS_SCOPE_STREAM_FIXTURE") {
        std::fs::write(path, format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>", bootstrap.head_script().unwrap(), shell_html, patch.to_html(&bootstrap).unwrap(), end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
