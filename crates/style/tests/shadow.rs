use glory_style::{
    BoxShadow, Color, Profile, Property, Shadow, Style, Support, Unsupported, Value,
};
fn shadow() -> Shadow {
    Shadow {
        x: -2.0,
        y: 3.0,
        blur: 4.0,
        spread: -1.0,
        color: Color(10, 20, 30, 40),
    }
}
#[test]
fn shadow_numeric_domains_reject_each_nonfinite_field_and_negative_blur() {
    Style::new().box_shadow(BoxShadow::None).validate().unwrap();
    Style::new()
        .box_shadow(BoxShadow::Outset(shadow()))
        .validate()
        .unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..4 {
            let mut s = shadow();
            match field {
                0 => s.x = invalid,
                1 => s.y = invalid,
                2 => s.blur = invalid,
                _ => s.spread = invalid,
            }
            assert!(
                Style::new()
                    .box_shadow(BoxShadow::Outset(s))
                    .validate()
                    .is_err()
            );
        }
    }
    let mut s = shadow();
    s.blur = -1.0;
    assert!(
        Style::new()
            .box_shadow(BoxShadow::Outset(s))
            .validate()
            .is_err()
    );
}
#[test]
fn shadow_property_and_each_value_and_px_unit_are_rejected_independently() {
    for (shadow, value) in [
        (BoxShadow::None, Value::ShadowNone),
        (BoxShadow::Outset(shadow()), Value::ShadowOutset),
    ] {
        let style = Style::new().box_shadow(shadow);
        Profile::HTML.inline.check(&style).unwrap();
        Profile::RECORD.inline.check(&style).unwrap();
        assert_eq!(
            style.properties().collect::<Vec<_>>(),
            [Property::BoxShadow]
        );
        assert_eq!(
            Support::ALL
                .without_property(Property::BoxShadow)
                .check(&style),
            Err(Unsupported {
                property: Property::BoxShadow,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(value).check(&style),
            Err(Unsupported {
                property: Property::BoxShadow,
                value: Some(value)
            })
        );
    }
    assert_eq!(
        Support::ALL
            .without_value(Value::Px)
            .check(&Style::new().box_shadow(BoxShadow::Outset(shadow()))),
        Err(Unsupported {
            property: Property::BoxShadow,
            value: Some(Value::Px)
        })
    );
    assert!(
        Support::ALL
            .without_value(Value::Px)
            .check(&Style::new().box_shadow(BoxShadow::None))
            .is_ok()
    );
    assert_eq!((Profile::HTML.version, Profile::RECORD.version), (9, 9));
}
#[cfg(feature = "css")]
#[test]
fn finite_css_shadows_keep_declaration_order_and_reject_unrepresented_syntax() {
    use glory_style::Sheet;
    let sheet = Sheet::parse(
        ".card{box-shadow:0 2px #0003;box-shadow:-2px 3px 4px -1px #0a141e28;box-shadow:none}",
    )
    .unwrap();
    let declarations = sheet.rules()[0].declarations();
    assert_eq!(declarations.len(), 3);
    assert_eq!(
        declarations[0].style().box_shadow,
        Some(BoxShadow::Outset(Shadow {
            x: 0.0,
            y: 2.0,
            blur: 0.0,
            spread: 0.0,
            color: Color(0, 0, 0, 51)
        }))
    );
    assert_eq!(
        declarations[1].style().box_shadow,
        Some(BoxShadow::Outset(shadow()))
    );
    assert_eq!(declarations[2].style().box_shadow, Some(BoxShadow::None));
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::ShadowOutset),
                ..Profile::HTML
            })
            .is_err(),
        "an overwritten original shadow must still be checked"
    );
    for css in [
        "inset 0 2px 4px black",
        "0 2px 4px black,0 1px red",
        "0 2px 4px",
        "0 2px -1px black",
        "0 2% 4px black",
        "0 2px 4px currentColor",
        "0 2px 4px black unexpected",
    ] {
        assert!(
            Sheet::parse(&format!(".card{{box-shadow:{css}}}")).is_err(),
            "{css}"
        );
    }
}
