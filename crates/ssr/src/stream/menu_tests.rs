use super::*;
use glory_render::{
    CheckMode, FocusMode, Layer, LayerPlacement, MenuItemKind, MenuItemSemantics, Orientation,
    PopupKind, Role, SemanticId, Semantics,
};

#[test]
fn menu_stream_retains_intrinsic_roles_and_lowers_popup_layer_and_controlled_properties() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "menu",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            let owner = SemanticId::new("stream-menu").unwrap();
            let trigger = SemanticId::new("stream-menu-trigger").unwrap();
            let trigger_id = trigger.clone();
            let panel_id = owner.clone();
            let check_owner = owner.clone();
            let radio_owner = owner.clone();
            let check = glory_ui::check()
                .label("Stream check")
                .checked(ready)
                .semantics(Some(Box::new(Semantics {
                    menu_item: Some(MenuItemSemantics {
                        kind: MenuItemKind::Checkbox,
                        owner: check_owner,
                    }),
                    ..Default::default()
                })));
            let radio = glory_ui::check()
                .mode(CheckMode::Radio)
                .label("Stream radio")
                .checked(ready)
                .semantics(Some(Box::new(Semantics {
                    menu_item: Some(MenuItemSemantics {
                        kind: MenuItemKind::Radio,
                        owner: radio_owner,
                    }),
                    check_group: Some(SemanticId::new("stream-radio-group").unwrap()),
                    ..Default::default()
                })));
            view().fill((
                view()
                    .role(Role::Menubar)
                    .name("Stream application")
                    .semantics(Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-menubar").unwrap()),
                        navigation: Some(Orientation::Horizontal),
                        ..Default::default()
                    })))
                    .fill(
                        glory_ui::button()
                            .label("Stream File")
                            .focus(FocusMode::TabStop)
                            .semantics(scope.bond(move || {
                                Some(Box::new(Semantics {
                                    id: Some(SemanticId::new("stream-menubar-file").unwrap()),
                                    menu_item: Some(MenuItemSemantics {
                                        kind: MenuItemKind::Action,
                                        owner: SemanticId::new("stream-menubar").unwrap(),
                                    }),
                                    menubar_item: true,
                                    navigation_item: true,
                                    popup: Some(PopupKind::Menu),
                                    controls: Some(SemanticId::new("stream-menu").unwrap()),
                                    expanded: Some(ready.get()),
                                    ..Default::default()
                                }))
                            })),
                    ),
                view()
                    .role(Role::Group)
                    .name("Stream context area")
                    .focus(FocusMode::TabStop)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-context-region").unwrap()),
                            context_menu: Some(if ready.get() {
                                glory_render::ContextEditPolicy::Override
                            } else {
                                glory_render::ContextEditPolicy::Native
                            }),
                            ..Default::default()
                        }))
                    })),
                view()
                    .role(Role::Menu)
                    .name("Stream context actions")
                    .focus(FocusMode::Programmatic)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-context-panel").unwrap()),
                            hidden: true,
                            navigation: Some(Orientation::Vertical),
                            layer: Some(Layer {
                                modal: false,
                                anchor: Some(SemanticId::new("stream-context-region").unwrap()),
                                placement: LayerPlacement::ContextPoint(
                                    glory_render::ContextPoint {
                                        x: if ready.get() { 125.5 } else { 30.0 },
                                        y: if ready.get() { 211.25 } else { 40.0 },
                                    },
                                ),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    })),
                glory_ui::button()
                    .label("Stream trigger")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(trigger_id.clone()),
                            controls: Some(panel_id.clone()),
                            popup: Some(if ready.get() {
                                PopupKind::Menu
                            } else {
                                PopupKind::Dialog
                            }),
                            expanded: Some(ready.get()),
                            ..Default::default()
                        }))
                    })),
                view()
                    .role(Role::Menu)
                    .name("Stream menu")
                    .focus(FocusMode::Programmatic)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(owner.clone()),
                            hidden: !ready.get(),
                            navigation: Some(Orientation::Vertical),
                            layer: Some(Layer {
                                modal: false,
                                placement: LayerPlacement::Anchor(
                                    glory_render::AnchorPlacement::new(
                                        glory_render::AnchorSide::Right,
                                        glory_render::AnchorAlign::Start,
                                    )
                                    .gap(4.0),
                                ),
                                anchor: Some(trigger.clone()),
                                return_focus: Some(trigger.clone()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .fill((check, radio)),
                glory_ui::button()
                    .label("Cleared popup")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            popup: (!ready.get()).then_some(PopupKind::Menu),
                            ..Default::default()
                        }))
                    })),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("role=\"menuitemcheckbox\""));
    assert!(shell.body().contains("role=\"menubar\""));
    assert!(shell.body().contains("data-gly-menubar-item"));
    assert!(shell.body().contains("aria-orientation=\"horizontal\""));
    assert!(shell.body().contains("role=\"menuitemradio\""));
    assert!(shell.body().contains("aria-haspopup=\"dialog\""));
    assert!(shell.body().contains("data-gly-context=\"native-editors\""));
    assert!(shell.body().contains("data-gly-context-x=\"30\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for marker in ["aria-haspopup", "menu", "hidden", "checked"] {
        assert!(patch.body().contains(marker), "missing {marker}");
    }
    for marker in ["override-editors", "data-gly-context-x", "125.5", "211.25"] {
        assert!(patch.body().contains(marker), "missing {marker}");
    }
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_MENU_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "menu-stream-root".into(),
            data_id: "menu-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path, format!(
            "<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",
            bootstrap.head_script().unwrap(), shell.to_html(&bootstrap).unwrap(),
            patch.to_html(&bootstrap).unwrap(), end.to_html(&bootstrap).unwrap()
        )).unwrap();
    }
}

#[test]
fn menu_search_stream_retains_the_editor_and_updates_the_active_relation_before_wasm() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "menu-search",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            view()
                .role(Role::Menu)
                .name("Search stream menu")
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("search-menu").unwrap()),
                    ..Default::default()
                })))
                .fill((
                    glory_ui::input()
                        .purpose(glory_render::Purpose::Search)
                        .name("Find stream action")
                        .semantics(scope.bond(move || {
                            Some(Box::new(Semantics {
                                id: Some(SemanticId::new("search-editor").unwrap()),
                                controls: Some(SemanticId::new("search-menu").unwrap()),
                                menu_search: Some(SemanticId::new("search-menu").unwrap()),
                                active_descendant: ready
                                    .get()
                                    .then(|| SemanticId::new("search-result").unwrap()),
                                ..Default::default()
                            }))
                        })),
                    glory_ui::button()
                        .label("Save stream document")
                        .semantics(Some(Box::new(Semantics {
                            id: Some(SemanticId::new("search-result").unwrap()),
                            menu_item: Some(MenuItemSemantics {
                                kind: MenuItemKind::Action,
                                owner: SemanticId::new("search-menu").unwrap(),
                            }),
                            ..Default::default()
                        }))),
                ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(
        shell
            .body()
            .contains("data-gly-menu-search=\"search-menu\"")
    );
    assert!(shell.body().contains("type=\"search\""));
    assert!(!shell.body().contains("aria-activedescendant"));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("aria-activedescendant"));
    assert!(patch.body().contains("search-result"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_MENU_SEARCH_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "menu-search-stream-root".into(),
            data_id: "menu-search-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        let path = std::path::PathBuf::from(path);
        std::fs::write(&path, format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",
            bootstrap.head_script().unwrap(), shell.to_html(&bootstrap).unwrap(), patch.to_html(&bootstrap).unwrap(), end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
