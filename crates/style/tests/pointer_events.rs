use glory_style::{PointerEvents, Profile, Property, Style, Support, Unsupported, Value};
#[test]
fn pointer_targeting_property_and_both_values_reject_independently() {
    assert_eq!(PointerEvents::default(), PointerEvents::Auto);
    assert_eq!(Style::new().pointer_events, None);
    for (events, value) in [
        (PointerEvents::Auto, Value::Auto),
        (PointerEvents::None, Value::None),
    ] {
        let style = Style::new().pointer_events(events);
        style.validate().unwrap();
        assert_eq!(
            style.properties().collect::<Vec<_>>(),
            [Property::PointerEvents]
        );
        for profile in [Profile::HTML, Profile::RECORD] {
            profile.inline.check(&style).unwrap();
        }
        assert_eq!(
            Support::ALL
                .without_property(Property::PointerEvents)
                .check(&style),
            Err(Unsupported {
                property: Property::PointerEvents,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(value).check(&style),
            Err(Unsupported {
                property: Property::PointerEvents,
                value: Some(value)
            })
        );
    }
    assert_eq!((Profile::HTML.version, Profile::RECORD.version), (9, 9));
}
#[cfg(feature = "css")]
#[test]
fn original_pointer_declarations_are_checked_even_when_a_later_value_wins() {
    use glory_style::Sheet;
    let sheet = Sheet::parse(".mask{pointer-events:none;pointer-events:auto}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().pointer_events,
        Some(PointerEvents::None)
    );
    sheet.check(Profile::HTML).unwrap();
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_property(Property::PointerEvents),
                ..Profile::HTML
            })
            .is_err()
    );
    assert!(
        sheet
            .check(Profile {
                inline: Support::ALL.without_value(Value::None),
                ..Profile::HTML
            })
            .is_err()
    );
    for css in [
        "inherit",
        "all",
        "visiblePainted",
        "painted",
        "auto none",
        "",
        "url(a)",
    ] {
        assert!(
            Sheet::parse(&format!(".mask{{pointer-events:{css}}}")).is_err(),
            "{css}"
        );
    }
}
