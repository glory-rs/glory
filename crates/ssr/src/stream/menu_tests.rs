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
                                placement: LayerPlacement::AnchorRight,
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
    assert!(shell.body().contains("role=\"menuitemradio\""));
    assert!(shell.body().contains("aria-haspopup=\"dialog\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for marker in ["aria-haspopup", "menu", "hidden", "checked"] {
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
