use super::*;
use glory_render::{
    ComboboxKind, FocusMode, Layer, LayerFocus, LayerPlacement, OptionSemantics, Orientation,
    PopupKind, Role, SemanticId, Semantics,
};

#[test]
fn choice_stream_retains_focus_preserving_intrinsics_and_live_active_descendant() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "choice",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            let trigger_id = SemanticId::new("stream-choice-trigger").unwrap();
            let listbox_id = SemanticId::new("stream-choice-listbox").unwrap();
            let option_id = SemanticId::new("stream-choice-beta").unwrap();
            let trigger = trigger_id.clone();
            let owner = listbox_id.clone();
            let active = option_id.clone();
            let panel_owner = listbox_id.clone();
            let panel_trigger = trigger_id.clone();
            let editor_owner = listbox_id.clone();
            let editor_active = option_id.clone();
            let hidden_owner = listbox_id.clone();
            view().fill((
                glory_ui::button()
                    .label("Stream choice")
                    .name("Stream choice")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(trigger.clone()),
                            controls: Some(owner.clone()),
                            combobox: Some(ComboboxKind::Select),
                            popup: Some(PopupKind::Listbox),
                            expanded: Some(ready.get()),
                            active_descendant: ready.get().then(|| active.clone()),
                            ..Default::default()
                        }))
                    })),
                view()
                    .role(Role::Listbox)
                    .name("Stream choices")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(panel_owner.clone()),
                            hidden: !ready.get(),
                            navigation: Some(Orientation::Vertical),
                            layer: Some(Layer {
                                modal: false,
                                focus: LayerFocus::Preserve,
                                placement: LayerPlacement::AnchorStart,
                                anchor: Some(panel_trigger.clone()),
                                initial_focus: None,
                                return_focus: None,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .fill((
                        glory_ui::button()
                            .label("Beta")
                            .focus(FocusMode::Programmatic)
                            .semantics(scope.bond(move || {
                                Some(Box::new(Semantics {
                                    id: Some(option_id.clone()),
                                    option: Some(OptionSemantics {
                                        owner: listbox_id.clone(),
                                    }),
                                    selected: Some(ready.get()),
                                    ..Default::default()
                                }))
                            })),
                        glory_ui::button()
                            .label("Alpha")
                            .focus(FocusMode::Programmatic)
                            .semantics(scope.bond(move || {
                                Some(Box::new(Semantics {
                                    id: Some(SemanticId::new("stream-choice-alpha").unwrap()),
                                    option: Some(OptionSemantics {
                                        owner: hidden_owner.clone(),
                                    }),
                                    selected: Some(false),
                                    hidden: ready.get(),
                                    ..Default::default()
                                }))
                            })),
                    )),
                glory_ui::input()
                    .name("Stream editable choice")
                    .value(scope.bond(move || {
                        if ready.get() {
                            "Beta".to_owned()
                        } else {
                            "Be".to_owned()
                        }
                    }))
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-choice-editor").unwrap()),
                            controls: Some(editor_owner.clone()),
                            combobox: Some(ComboboxKind::Editable),
                            popup: Some(PopupKind::Listbox),
                            expanded: Some(ready.get()),
                            active_descendant: ready.get().then(|| editor_active.clone()),
                            ..Default::default()
                        }))
                    })),
                glory_ui::input()
                    .name("Stream autocomplete")
                    .value(scope.bond(move || {
                        if ready.get() {
                            "Free city".to_owned()
                        } else {
                            "Draft city".to_owned()
                        }
                    }))
                    .semantics(Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-autocomplete").unwrap()),
                        controls: Some(SemanticId::new("stream-choice-listbox").unwrap()),
                        combobox: Some(ComboboxKind::Autocomplete),
                        popup: Some(PopupKind::Listbox),
                        expanded: Some(false),
                        required: true,
                        form_control: Some(glory_render::FormControl {
                            name: "free-city".into(),
                            value: None,
                        }),
                        ..Default::default()
                    }))),
                glory_ui::input()
                    .name("Stream multiple choice")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-multiple").unwrap()),
                            controls: Some(SemanticId::new("stream-multiple-listbox").unwrap()),
                            combobox: Some(ComboboxKind::Multiple),
                            popup: Some(PopupKind::Listbox),
                            expanded: Some(ready.get()),
                            active_descendant: ready
                                .get()
                                .then(|| SemanticId::new("stream-multiple-option").unwrap()),
                            required: true,
                            ..Default::default()
                        }))
                    })),
                view()
                    .role(Role::Listbox)
                    .name("Stream multiple options")
                    .semantics(Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-multiple-listbox").unwrap()),
                        listbox_multiple: true,
                        navigation: Some(Orientation::Vertical),
                        ..Default::default()
                    })))
                    .fill(
                        glory_ui::button()
                            .label("Multiple Alpha")
                            .focus(FocusMode::Programmatic)
                            .semantics(scope.bond(move || {
                                Some(Box::new(Semantics {
                                    id: Some(SemanticId::new("stream-multiple-option").unwrap()),
                                    option: Some(OptionSemantics {
                                        owner: SemanticId::new("stream-multiple-listbox").unwrap(),
                                    }),
                                    selected: Some(ready.get()),
                                    ..Default::default()
                                }))
                            })),
                    ),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    for marker in [
        "role=\"combobox\"",
        "role=\"listbox\"",
        "role=\"option\"",
        "data-gly-combobox=\"select\"",
        "data-gly-combobox=\"editable\"",
        "data-gly-combobox=\"autocomplete\"",
        "data-gly-combobox=\"multiple\"",
        "aria-multiselectable=\"true\"",
        "tabindex=\"-1\"",
    ] {
        assert!(shell.body().contains(marker), "missing {marker}");
    }
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    for marker in [
        "aria-activedescendant",
        "stream-choice-beta",
        "aria-selected",
        "aria-expanded",
        "hidden",
    ] {
        assert!(patch.body().contains(marker), "missing {marker}");
    }
    let end = frame(&mut stream);
    assert!(end.body().contains("\"html_version\":4"));
    if let Some(path) = std::env::var_os("GLORY_CHOICE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "choice-stream-root".into(),
            data_id: "choice-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path, format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>", bootstrap.head_script().unwrap(), shell.to_html(&bootstrap).unwrap(), patch.to_html(&bootstrap).unwrap(), end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
