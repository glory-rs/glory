use super::*;
use glory_render::{SelectEntry, SelectOption, SelectProps, SemanticId};

#[test]
fn select_stream_uses_v4_shape_v2_envelope_and_preserves_native_selection_through_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "select",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            let entries = std::rc::Rc::new(
                [("a", "Alpha", "business-a"), ("b", "Beta", "business-b")]
                    .into_iter()
                    .map(|(key, label, value)| {
                        SelectEntry::Option(SelectOption {
                            key: SemanticId::new(key).unwrap(),
                            label: label.into(),
                            value: value.into(),
                            disabled: false,
                        })
                    })
                    .collect(),
            );
            glory_ui::select()
                .name("Stream native choice")
                .configuration(SelectProps {
                    entries,
                    placeholder: "Choose".into(),
                    ..Default::default()
                })
                .selected(scope.bond(move || {
                    if ready.get() {
                        vec![SemanticId::new("b").unwrap()]
                    } else {
                        vec![]
                    }
                }))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().starts_with("<!--gly:stream:2-->"));
    assert!(
        shell.body().contains("<select")
            && shell.body().contains("data-gly-placeholder")
            && shell.body().contains("business-a")
    );
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("loaded".into()));
    let patch = frame(&mut stream);
    assert!(patch.body().contains("selectedKeys") && patch.body().contains("\"version\":2"));
    let end = frame(&mut stream);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_SELECT_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "select-stream-root".into(),
            data_id: "select-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
