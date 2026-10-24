use super::*;
use glory_render::{Direction, Environment, LanguageTag, Semantics};

#[test]
fn environment_direction_language_and_classic_start_end_share_the_current_format() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "environment",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            view()
                .semantics(scope.bond(move || {
                    let ready = ready.get();
                    Some(Box::new(Semantics {
                        environment: Some(Environment {
                            direction: if ready {
                                Direction::Rtl
                            } else {
                                Direction::Ltr
                            },
                            language: LanguageTag::new(if ready { "ar" } else { "en-US" }).unwrap(),
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(
                    glory_ui::input()
                        .name("Environment stream draft")
                        .value("Draft 🙂"),
                )
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("dir=\"ltr\""));
    assert!(shell.body().contains("lang=\"en-US\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert!(
        patch.body().contains("\"name\":\"dir\"") && patch.body().contains("\"value\":\"rtl\"")
    );
    assert!(
        patch.body().contains("\"name\":\"lang\"") && patch.body().contains("\"value\":\"ar\"")
    );
    let end = frame(&mut stream);
    let bootstrap = Bootstrap {
        root_id: "environment-stream-root".into(),
        data_id: "environment-stream-data".into(),
        script_url: "glory-stream.js".into(),
        nonce: None,
    };
    let shell_html = shell.to_html(&bootstrap).unwrap();
    assert!(shell_html.contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION)));
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_ENVIRONMENT_STREAM_FIXTURE") {
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell_html,patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
