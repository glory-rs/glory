use glory_style::{Display, Profile, Property, Style, Support, Unsupported, Value};
#[test]
fn inline_flex_has_independent_value_and_shared_display_property_rejection() {
    let style = Style::new().display(Display::InlineFlex);
    Profile::HTML.inline.check(&style).unwrap();
    assert_eq!(
        Support::ALL
            .without_property(Property::Display)
            .check(&style),
        Err(Unsupported {
            property: Property::Display,
            value: None
        })
    );
    assert_eq!(
        Support::ALL.without_value(Value::InlineFlex).check(&style),
        Err(Unsupported {
            property: Property::Display,
            value: Some(Value::InlineFlex)
        })
    );
    Support::ALL
        .without_value(Value::Flex)
        .check(&style)
        .unwrap();
    assert_eq!(Display::default(), Display::Block);
}
#[cfg(feature = "css")]
#[test]
fn finite_css_parses_inline_flex_without_widening_other_display_values() {
    let sheet = glory_style::Sheet::parse(".pill{display:inline-flex}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().display,
        Some(Display::InlineFlex)
    );
    for display in ["inline", "inline-block", "inline-grid", "contents"] {
        assert!(glory_style::Sheet::parse(&format!(".pill{{display:{display}}}")).is_err());
    }
}
