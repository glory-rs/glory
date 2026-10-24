use super::*;
use glory_render::{ProgressValue, Role, Semantics};
#[test]
fn read_only_progress_numeric_and_indeterminate_metadata_survives_stream_patch_and_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "progress",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view()
                .role(Role::ProgressBar)
                .name("Streaming upload")
                .semantics(ready.map(|state| {
                    Some(Box::new(Semantics {
                        progress: Some(ProgressValue {
                            min: 0.,
                            max: 100.,
                            current: if matches!(state, ResourceState::Ready { .. }) {
                                None
                            } else {
                                Some(25.)
                            },
                            value_text: Some(
                                if matches!(state, ResourceState::Ready { .. }) {
                                    "Waiting"
                                } else {
                                    "Quarter"
                                }
                                .into(),
                            ),
                        }),
                        ..Default::default()
                    }))
                }))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(
        shell.body().contains("role=\"progressbar\"")
            && shell.body().contains("aria-valuenow=\"25\"")
    );
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(
        patch
            .body()
            .contains("\"name\":\"aria-valuenow\",\"part\":\"main\",\"value\":null"),
        "{}",
        patch.body()
    );
    assert!(patch.body().contains("Waiting"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_PROGRESS_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "progress-stream-root".into(),
            data_id: "progress-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
