use glory_style::{Length, Profile, Property, Style, Support, Unsupported, Value};

fn declarations(v: f64) -> [(Property, Style); 8] {
    let ch = Length::Ch(v);
    [
        (Property::Width, Style::new().width(ch)),
        (Property::Height, Style::new().height(ch)),
        (Property::MinWidth, Style::new().min_width(ch)),
        (Property::MinHeight, Style::new().min_height(ch)),
        (Property::Top, Style::new().top(ch)),
        (Property::Left, Style::new().left(ch)),
        (Property::Right, Style::new().right(ch)),
        (Property::Bottom, Style::new().bottom(ch)),
    ]
}
#[test]
fn ch_uses_the_last_value_bit_and_each_property_is_independently_rejectable() {
    assert_eq!(Value::Ch as u8, 63);
    assert_eq!(Value::ALL.len(), 64);
    assert_eq!((Profile::HTML.version, Profile::RECORD.version), (10, 10));
    for (property, style) in declarations(6.) {
        style.validate().unwrap();
        for profile in [Profile::HTML, Profile::RECORD] {
            profile.inline.check(&style).unwrap();
        }
        assert_eq!(
            Support::ALL.without_property(property).check(&style),
            Err(Unsupported {
                property,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(Value::Ch).check(&style),
            Err(Unsupported {
                property,
                value: Some(Value::Ch)
            })
        );
    }
}
#[test]
fn ch_coordinates_allow_negative_offsets_but_dimensions_remain_nonnegative_and_finite() {
    for (property, style) in declarations(-1.) {
        assert_eq!(
            style.validate().is_ok(),
            matches!(
                property,
                Property::Top | Property::Left | Property::Right | Property::Bottom
            )
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (_, style) in declarations(value) {
            assert!(style.validate().is_err());
        }
    }
    for (_, style) in declarations(0.) {
        style.validate().unwrap();
    }
}
#[cfg(feature = "css")]
#[test]
fn optional_css_preserves_ch_and_checks_overridden_originals_without_widening_other_domains() {
    use glory_style::Sheet;
    let sheet = Sheet::parse(".code{width:6ch;left:-1ch;width:20px}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().width,
        Some(Length::Ch(6.))
    );
    sheet.check(Profile::HTML).unwrap();
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::Ch),
                ..Profile::HTML
            })
            .is_err()
    );
    for css in [
        ".a{max-width:6ch}",
        ".a{max-height:6ch}",
        ".a{padding:1ch}",
        ".a{gap:1ch}",
        ".a{grid-template-columns:1ch}",
        ".a{width:calc(6ch)}",
        ".a{width:-1ch}",
    ] {
        assert!(Sheet::parse(css).is_err(), "{css}");
    }
}
