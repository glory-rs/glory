use glory_style::{Profile, Property, Style, Support, TextOverflow, Unsupported, Value};
#[test]
fn each_text_overflow_value_and_the_property_have_independent_rejection() {
    for (overflow, value) in [
        (TextOverflow::Clip, Value::TextClip),
        (TextOverflow::Ellipsis, Value::TextEllipsis),
    ] {
        let style = Style::new().text_overflow(overflow);
        style.validate().unwrap();
        Profile::HTML.inline.check(&style).unwrap();
        Profile::RECORD.inline.check(&style).unwrap();
        assert_eq!(
            Support::ALL
                .without_property(Property::TextOverflow)
                .check(&style),
            Err(Unsupported {
                property: Property::TextOverflow,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(value).check(&style),
            Err(Unsupported {
                property: Property::TextOverflow,
                value: Some(value)
            })
        );
    }
}
#[cfg(feature = "css")]
#[test]
fn finite_text_overflow_css_checks_original_declarations_and_rejects_other_grammars() {
    let sheet =
        glory_style::Sheet::parse(".text{text-overflow:ellipsis;text-overflow:clip}").unwrap();
    let declarations = sheet.rules()[0].declarations();
    assert_eq!(
        declarations[0].style().text_overflow,
        Some(TextOverflow::Ellipsis)
    );
    assert_eq!(
        declarations[1].style().text_overflow,
        Some(TextOverflow::Clip)
    );
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::TextEllipsis),
                ..Profile::HTML
            })
            .is_err()
    );
    for value in ["fade", "ellipsis clip", "'x'", "ellipsis extra"] {
        assert!(glory_style::Sheet::parse(&format!(".text{{text-overflow:{value}}}")).is_err());
    }
}
