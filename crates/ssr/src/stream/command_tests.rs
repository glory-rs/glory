use super::*;
use glory_render::{
    ComboboxKind, FocusMode, Layer, LayerPlacement, OptionSemantics, Orientation, PopupKind, Role,
    SemanticId, Semantics,
};
#[test]
fn command_dialog_editor_grouped_options_and_pending_states_survive_classic_stream_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream =
        stream_with(
            Resources::default(),
            Config::default(),
            move |scope, resources| {
                let resource = resources
                    .bind(
                        scope.owner(),
                        "commands",
                        "v1",
                        || 1_u64,
                        move |_| loader.waiting(),
                    )
                    .unwrap();
                let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
                let sid = |value| SemanticId::new(value).unwrap();
                view().on(glory_render::EventKind::Palette, |_| {}).fill((
                    glory_ui::button()
                        .label("Open stream commands")
                        .semantics(scope.bond(move || {
                            Some(Box::new(Semantics {
                                id: Some(sid("stream-command-trigger")),
                                controls: Some(sid("stream-command-dialog")),
                                popup: Some(PopupKind::Dialog),
                                expanded: Some(ready.get()),
                                ..Default::default()
                            }))
                        })),
                    view()
                        .role(Role::Dialog)
                        .name("Stream commands")
                        .focus(FocusMode::Programmatic)
                        .semantics(scope.bond(move || {
                            Some(Box::new(Semantics {
                                id: Some(sid("stream-command-dialog")),
                                hidden: !ready.get(),
                                layer: Some(Layer {
                                    modal: true,
                                    placement: LayerPlacement::Center,
                                    initial_focus: Some(sid("stream-command-editor")),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }))
                        }))
                        .fill((
                            glory_ui::input()
                                .name("Search stream commands")
                                .value(scope.bond(move || {
                                    if ready.get() { "files" } else { "" }.to_string()
                                }))
                                .semantics(scope.bond(move || {
                                    Some(Box::new(Semantics {
                                        id: Some(sid("stream-command-editor")),
                                        combobox: Some(ComboboxKind::Editable),
                                        controls: Some(sid("stream-command-list")),
                                        popup: Some(PopupKind::Listbox),
                                        expanded: Some(ready.get()),
                                        active_descendant: ready
                                            .get()
                                            .then(|| sid("stream-command-option")),
                                        ..Default::default()
                                    }))
                                })),
                            view()
                                .role(Role::Listbox)
                                .name("Stream command results")
                                .semantics(Some(Box::new(Semantics {
                                    id: Some(sid("stream-command-list")),
                                    navigation: Some(Orientation::Vertical),
                                    ..Default::default()
                                })))
                                .fill(
                                    view().role(Role::Group).name("Files").fill(
                                        glory_ui::button()
                                            .label("New stream document")
                                            .focus(FocusMode::Programmatic)
                                            .enabled(ready)
                                            .semantics(scope.bond(move || {
                                                Some(Box::new(Semantics {
                                                    id: Some(sid("stream-command-option")),
                                                    option: Some(OptionSemantics {
                                                        owner: sid("stream-command-list"),
                                                    }),
                                                    selected: Some(ready.get()),
                                                    ..Default::default()
                                                }))
                                            })),
                                    ),
                                ),
                            view()
                                .role(Role::Status)
                                .fill(text().content(scope.bond(move || {
                                    if ready.get() {
                                        "One command"
                                    } else {
                                        "Loading commands"
                                    }
                                    .to_string()
                                }))),
                        )),
                ))
            },
        )
        .unwrap();
    let shell = frame(&mut stream);
    assert!(
        shell.body().contains("role=\"dialog\"")
            && shell.body().contains("role=\"combobox\"")
            && shell.body().contains("role=\"listbox\"")
            && shell.body().contains("role=\"option\"")
    );
    assert!(shell.body().contains("Loading commands"));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for marker in [
        "hidden",
        "aria-activedescendant",
        "stream-command-option",
        "files",
        "One command",
    ] {
        assert!(patch.body().contains(marker), "{marker}")
    }
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_COMMAND_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "command-stream-root".into(),
            data_id: "command-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
