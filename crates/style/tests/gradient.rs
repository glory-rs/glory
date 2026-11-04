#[cfg(feature = "css")]
use glory_style::Sheet;
use glory_style::{Color, LinearGradient, Profile, Property, Style, Support, Unsupported};

fn gradient() -> LinearGradient {
    LinearGradient {
        angle: 90.0,
        stops: vec![Color(0, 0, 0, 0), Color(34, 139, 230, 255)],
    }
}

#[test]
fn linear_gradients_validate_and_have_independent_property_support() {
    let style = Style::new().background_image(gradient());
    assert!(style.validate().is_ok());
    assert!(Profile::HTML.inline.check(&style).is_ok());
    assert!(Profile::RECORD.inline.check(&style).is_ok());
    assert_eq!(
        style.properties().collect::<Vec<_>>(),
        [Property::BackgroundImage]
    );
    assert_eq!(
        Support::ALL
            .without_property(Property::BackgroundImage)
            .check(&style),
        Err(Unsupported {
            property: Property::BackgroundImage,
            value: None,
        })
    );
    for angle in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            Style::new()
                .background_image(LinearGradient {
                    angle,
                    ..gradient()
                })
                .validate()
                .is_err()
        );
    }
    for stops in [vec![], vec![Color(0, 0, 0, 0)]] {
        assert!(
            Style::new()
                .background_image(LinearGradient {
                    stops,
                    ..gradient()
                })
                .validate()
                .is_err()
        );
    }
}

#[cfg(feature = "css")]
#[test]
fn stylesheet_accepts_only_the_typed_linear_gradient_subset() {
    let sheet =
        Sheet::parse(".a{background-image:linear-gradient(90deg,transparent,#228be6ff)}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().background_image,
        Some(gradient())
    );
    sheet.check(Profile::HTML).unwrap();
    for css in [
        ".a{background-image:none}",
        ".a{background-image:linear-gradient(to right,red,blue)}",
        ".a{background-image:linear-gradient(90deg,red)}",
        ".a{background-image:radial-gradient(red,blue)}",
    ] {
        assert!(Sheet::parse(css).is_err(), "{css}");
    }
}
