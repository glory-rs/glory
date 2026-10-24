use super::*;
use glory_render::{Change, EventKind, FileConfig, FileFilter, Patch};
#[test]
fn file_root_picker_siblings_config_patch_and_end_rekey_stay_in_one_current_shape() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "files",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = ready.map(|state| matches!(state, ResourceState::Ready { .. }));
            widget(move |scope| {
                let reference = scope.cage(None);
                glory_ui::file()
                    .label("Stream file picker")
                    .class("stream-file")
                    .node_ref(reference)
                    .on(EventKind::Files, |_| {})
                    .show_in(scope)?;
                let weak = scope.downgrade();
                let mut updated = false;
                scope.structural(move || {
                    if ready.get()
                        && !updated
                        && let Some(node) = reference.get()
                        && let Some(scope) = weak.upgrade()
                    {
                        let result = scope.patch(
                            node,
                            Patch::File {
                                configuration: Change::Set(Box::new(FileConfig {
                                    multiple: true,
                                    drop: true,
                                    max_files: 2,
                                    filters: vec![FileFilter::Extension(".txt".into())],
                                    ..Default::default()
                                })),
                                revision: 1,
                            },
                        );
                        match result {
                            Ok(()) => updated = true,
                            Err(error) => scope.report_error(error),
                        }
                    }
                });
                Ok(())
            })
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(
        shell
            .body()
            .contains("data-gly-part=\"control\" style=\"display:contents;\"")
    );
    assert!(shell.body().contains("</button><input "));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for value in ["picker", "multiple", "accept", ".txt", "data-gly-file-drop"] {
        assert!(patch.body().contains(value), "{value}");
    }
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_FILES_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "files-stream-root".into(),
            data_id: "files-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
