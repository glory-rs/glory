use glory_style::{Color, LinearGradient, Profile, Property, Style, Support, Unsupported};

fn gradient() -> LinearGradient {
    LinearGradient {
        angle: 75.0,
        stops: vec![Color(20, 80, 220, 255), Color(190, 40, 210, 255)],
    }
}

#[test]
fn line_clamp_and_text_gradient_are_independent_bounded_properties() {
    let style = Style::new().line_clamp(3).text_gradient(gradient());
    assert!(style.validate().is_ok());
    assert!(Profile::HTML.inline.check(&style).is_ok());
    for property in [Property::LineClamp, Property::TextGradient] {
        assert_eq!(
            Support::ALL.without_property(property).check(&style),
            Err(Unsupported {
                property,
                value: None,
            })
        );
    }
    assert!(Style::new().line_clamp(0).validate().is_err());
    assert!(Style::new().line_clamp(257).validate().is_err());
    assert!(
        Style::new()
            .text_gradient(LinearGradient {
                angle: f64::NAN,
                stops: gradient().stops,
            })
            .validate()
            .is_err()
    );
}
