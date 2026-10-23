use glory_style::{FlexWrap, Profile, Property, Style, Support, Unsupported, Value};
#[test]
fn wrap_property_and_both_values_have_independent_rejection() {
    for (wrap, value) in [
        (FlexWrap::NoWrap, Value::FlexNoWrap),
        (FlexWrap::Wrap, Value::FlexWrap),
    ] {
        let style = Style::new().flex_wrap(wrap);
        style.validate().unwrap();
        Profile::HTML.inline.check(&style).unwrap();
        assert_eq!(
            Support::ALL
                .without_property(Property::FlexWrap)
                .check(&style),
            Err(Unsupported {
                property: Property::FlexWrap,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(value).check(&style),
            Err(Unsupported {
                property: Property::FlexWrap,
                value: Some(value)
            })
        );
    }
    assert_eq!(Style::new().flex_wrap, None);
    assert_eq!(
        Style::new().flex_wrap(FlexWrap::NoWrap).flex_wrap,
        Some(FlexWrap::NoWrap)
    );
}
#[cfg(feature = "css")]
#[test]
fn finite_wrap_css_preserves_both_values_and_rejects_wrap_reverse() {
    let sheet = glory_style::Sheet::parse(".row{flex-wrap:wrap;flex-wrap:nowrap}").unwrap();
    let declarations = sheet.rules()[0].declarations();
    assert_eq!(declarations[0].style().flex_wrap, Some(FlexWrap::Wrap));
    assert_eq!(declarations[1].style().flex_wrap, Some(FlexWrap::NoWrap));
    assert!(glory_style::Sheet::parse(".row{flex-wrap:wrap-reverse}").is_err());
}
