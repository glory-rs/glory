use glory_style::{Length, Position, Profile, Property, Style, Support, Unsupported, Value};
#[test]
fn fixed_and_both_end_insets_have_independent_property_and_unit_acceptance() {
    let fixed = Style::new().position(Position::Fixed);
    assert_eq!(
        Support::ALL.without_value(Value::Fixed).check(&fixed),
        Err(Unsupported {
            property: Property::Position,
            value: Some(Value::Fixed)
        })
    );
    for (property, style) in [
        (Property::Right, Style::new().right(Length::Px(-3.0))),
        (Property::Bottom, Style::new().bottom(Length::Percent(20.0))),
    ] {
        assert!(style.validate().is_ok());
        assert!(Profile::HTML.inline.check(&style).is_ok());
        assert_eq!(
            Support::ALL.without_property(property).check(&style),
            Err(Unsupported {
                property,
                value: None
            })
        );
    }
    for (unit, value) in [
        (Length::Auto, Value::Auto),
        (Length::Px(1.0), Value::Px),
        (Length::Percent(2.0), Value::Percent),
    ] {
        for (property, style) in [
            (Property::Right, Style::new().right(unit)),
            (Property::Bottom, Style::new().bottom(unit)),
        ] {
            assert_eq!(
                Support::ALL.without_value(value).check(&style),
                Err(Unsupported {
                    property,
                    value: Some(value)
                })
            );
        }
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Style::new().right(Length::Px(value)).validate().is_err());
        assert!(
            Style::new()
                .bottom(Length::Percent(value))
                .validate()
                .is_err()
        );
    }
}
#[cfg(feature = "css")]
#[test]
fn parser_checks_all_original_fixed_and_inset_declarations_before_overrides() {
    use glory_style::Sheet;
    let sheet = Sheet::parse(".a{position:fixed;right:-3px;bottom:20%;right:auto}").unwrap();
    assert!(sheet.check(Profile::HTML).is_ok());
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::Fixed),
                ..Profile::HTML
            })
            .is_err()
    );
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::Px),
                ..Profile::HTML
            })
            .is_err()
    );
    assert!(Sheet::parse(".a{right:calc(1px);bottom:inherit}").is_err());
}
