use glory_render::*;
#[test]
fn hints_belong_to_actual_single_line_inputs_including_password_and_preserve_intrinsic_roles() {
    for purpose in [
        Purpose::Text,
        Purpose::Password,
        Purpose::Number,
        Purpose::Email,
        Purpose::Url,
    ] {
        let mut node = Descriptor::new(Props::Input(InputProps {
            purpose,
            ..Default::default()
        }));
        node.semantics = Some(Box::new(Semantics {
            input_assist: Some(InputAssist {
                autofill: Some(InputAutofill::OneTimeCode),
                mode: Some(InputMode::Numeric),
            }),
            ..Default::default()
        }));
        node.validate().unwrap();
        assert_eq!(node.effective_role(), None);
        node.props = Props::Input(InputProps {
            purpose,
            multiline: true,
            ..Default::default()
        });
        assert!(node.validate().is_err());
    }
    for props in [
        Props::View,
        Props::Group,
        Props::Button(Default::default()),
        Props::Check(Default::default()),
    ] {
        let mut node = Descriptor::new(props);
        node.semantics = Some(Box::new(Semantics {
            input_assist: Some(InputAssist::default()),
            ..Default::default()
        }));
        assert!(node.validate().is_err());
    }
}
