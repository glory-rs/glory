use glory_render::*;
#[test]
fn native_radio_group_names_are_finite_and_successful_names_cannot_conflict() {
    let mut descriptor = Descriptor::new(Props::Check(CheckProps {
        mode: CheckMode::Radio,
        ..Default::default()
    }));
    let metadata = Semantics {
        check_group: Some(SemanticId::new("delivery").unwrap()),
        navigation_item: true,
        form_control: Some(FormControl {
            name: "delivery".into(),
            value: Some("express".into()),
        }),
        ..Default::default()
    };
    descriptor
        .patch(Patch::Semantics(Change::Set(Some(Box::new(
            metadata.clone(),
        )))))
        .unwrap();
    let before = descriptor.clone();
    let mut conflicting = metadata.clone();
    conflicting.form_control.as_mut().unwrap().name = "other".into();
    assert!(
        descriptor
            .patch(Patch::Semantics(Change::Set(Some(Box::new(conflicting)))))
            .is_err()
    );
    assert_eq!(descriptor, before);
    assert!(descriptor.patch(Patch::CheckMode(Change::Clear)).is_err());
    assert_eq!(descriptor, before);
    let mut checkbox = Descriptor::new(Props::Check(CheckProps::default()));
    assert!(
        checkbox
            .patch(Patch::Semantics(Change::Set(Some(Box::new(metadata)))))
            .is_err()
    );
    let mut group = Descriptor::new(Props::View);
    group.role = Some(Role::RadioGroup);
    group.semantics = Some(Box::new(Semantics {
        navigation: Some(Orientation::Vertical),
        required: true,
        readonly: true,
        invalid: true,
        ..Default::default()
    }));
    group.validate().unwrap();
}
