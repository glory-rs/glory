use glory_render::*;
#[test]
fn pressed_is_a_boolean_native_button_state_and_conflicts_are_transactional() {
    let mut button = Descriptor::new(Props::Button(ButtonProps::default()));
    for pressed in [false, true] {
        button
            .patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                pressed: Some(pressed),
                ..Default::default()
            })))))
            .unwrap();
    }
    let before = button.clone();
    for metadata in [
        Semantics {
            pressed: Some(true),
            tab: true,
            selected: Some(true),
            ..Default::default()
        },
        Semantics {
            pressed: Some(false),
            popup: Some(glory_render::PopupKind::Dialog),
            ..Default::default()
        },
        Semantics {
            pressed: Some(false),
            form_action: Some(FormAction::Submit),
            ..Default::default()
        },
    ] {
        assert!(
            button
                .patch(Patch::Semantics(Change::Set(Some(Box::new(metadata)))))
                .is_err()
        );
        assert_eq!(button, before);
    }
    for props in [
        Props::View,
        Props::Group,
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
        Props::Text(TextProps::default()),
    ] {
        let mut descriptor = Descriptor::new(props);
        let before = descriptor.clone();
        assert!(
            descriptor
                .patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                    pressed: Some(true),
                    ..Default::default()
                })))))
                .is_err()
        );
        assert_eq!(descriptor, before);
    }
    button
        .patch(Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            pressed: Some(true),
            expanded: Some(true),
            ..Default::default()
        })))))
        .unwrap();
}
