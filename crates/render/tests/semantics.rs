use glory_render::*;

#[test]
fn structural_roles_do_not_replace_the_intrinsic_semantics_of_controls_text_or_images() {
    for props in [
        Props::Text(TextProps::default()),
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
        Props::Check(CheckProps::default()),
        Props::Image(ImageProps::default()),
        Props::Link(LinkProps::default()),
    ] {
        let mut node = Descriptor::new(props);
        let before = node.clone();
        for role in [
            Role::Group,
            Role::Main,
            Role::Section,
            Role::Navigation,
            Role::List,
            Role::ListItem,
            Role::Status,
            Role::Alert,
            Role::Table,
            Role::Row,
            Role::ColumnHeader,
            Role::Cell,
            Role::Dialog,
        ] {
            assert_eq!(
                node.patch(Patch::Role(Change::Set(Some(role)))),
                Err(Error::InvalidProps("structural roles require a view"))
            );
            assert_eq!(node, before);
        }
        node.patch(Patch::Role(Change::Keep)).unwrap();
        node.patch(Patch::Role(Change::Clear)).unwrap();
        assert_eq!(node, before);
    }
}

#[test]
fn effective_view_roles_and_decorative_focus_have_explicit_reset_semantics() {
    let mut view = Descriptor::new(Props::View);
    assert_eq!(view.effective_role(), None);
    view.patch(Patch::Name(Change::Set(Some("Panel".into()))))
        .unwrap();
    assert_eq!(view.role, None);
    assert_eq!(view.effective_role(), Some(Role::Group));
    view.patch(Patch::Role(Change::Set(Some(Role::List))))
        .unwrap();
    assert_eq!(view.effective_role(), Some(Role::List));
    view.patch(Patch::Role(Change::Clear)).unwrap();
    assert_eq!(view.effective_role(), Some(Role::Group));
    view.patch(Patch::Name(Change::Clear)).unwrap();
    assert_eq!(view.effective_role(), None);
    let mut image = Descriptor::new(Props::Image(ImageProps::default()));
    let before = image.clone();
    for mode in [FocusMode::Programmatic, FocusMode::TabStop] {
        assert!(image.patch(Patch::Focus(Change::Set(mode))).is_err());
        assert_eq!(image, before);
    }
    let id = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        Action::Focus(id).validate_target(&image),
        Err(Error::Unsupported("focus on decorative image"))
    );
    image
        .patch(Patch::Alternative(Change::Set(Some(
            "visible image".into(),
        ))))
        .unwrap();
    image
        .patch(Patch::Focus(Change::Set(FocusMode::TabStop)))
        .unwrap();
    Action::Focus(id).validate_target(&image).unwrap();
    let before = image.clone();
    assert!(image.patch(Patch::Alternative(Change::Clear)).is_err());
    assert_eq!(image, before);
    image
        .patches([
            Patch::Focus(Change::Clear),
            Patch::Alternative(Change::Clear),
        ])
        .unwrap();
}

#[test]
fn author_names_cannot_be_silently_ignored_on_plain_text_or_decorative_images() {
    for props in [
        Props::Text(TextProps::default()),
        Props::Text(TextProps {
            kind: TextKind::Paragraph,
            ..Default::default()
        }),
        Props::Image(ImageProps::default()),
    ] {
        let mut node = Descriptor::new(props);
        let before = node.clone();
        assert!(
            node.patch(Patch::Name(Change::Set(Some("name".into()))))
                .is_err()
        );
        assert_eq!(node, before);
        node.patch(Patch::Name(Change::Keep)).unwrap();
        node.patch(Patch::Name(Change::Clear)).unwrap();
    }
    let mut heading = Descriptor::new(Props::Text(TextProps::default()));
    heading
        .patches([
            Patch::Name(Change::Set(Some("heading".into()))),
            Patch::TextKind(Change::Set(TextKind::Heading(2))),
        ])
        .unwrap();
    let before = heading.clone();
    assert!(heading.patch(Patch::TextKind(Change::Clear)).is_err());
    assert_eq!(heading, before);
    heading
        .patches([Patch::Name(Change::Clear), Patch::TextKind(Change::Clear)])
        .unwrap();

    let mut image = Descriptor::new(Props::Image(ImageProps {
        alternative: Some("description".into()),
        ..Default::default()
    }));
    image
        .patch(Patch::Name(Change::Set(Some("override".into()))))
        .unwrap();
    let before = image.clone();
    assert!(image.patch(Patch::Alternative(Change::Clear)).is_err());
    assert_eq!(image, before);
    image
        .patches([
            Patch::Name(Change::Clear),
            Patch::Alternative(Change::Clear),
        ])
        .unwrap();
}
