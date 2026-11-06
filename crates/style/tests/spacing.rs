use glory_style::{Length, Profile, Property, Style, Support, Unsupported, Value};

#[test]
fn finite_spacing_preserves_the_u64_domain_and_rejects_both_arithmetic_units_independently() {
    assert_eq!(Value::ALL.len(), 64);
    for (property, style) in [
        (
            Property::Width,
            Style::new().width(Length::ChPx { ch: 6., px: 144. }),
        ),
        (
            Property::Height,
            Style::new().height(Length::ChPx { ch: 1., px: 10. }),
        ),
        (
            Property::MinWidth,
            Style::new().min_width(Length::ChPx { ch: 2., px: 5. }),
        ),
        (
            Property::MinHeight,
            Style::new().min_height(Length::ChPx { ch: 1., px: 5. }),
        ),
        (
            Property::Top,
            Style::new().top(Length::ChPx { ch: -1., px: -4. }),
        ),
        (
            Property::Left,
            Style::new().left(Length::ChPx { ch: 1., px: 4. }),
        ),
        (
            Property::Right,
            Style::new().right(Length::ChPx { ch: 1., px: -4. }),
        ),
        (
            Property::Bottom,
            Style::new().bottom(Length::ChPx { ch: -1., px: 4. }),
        ),
    ] {
        style.validate().unwrap();
        for profile in [Profile::HTML, Profile::RECORD] {
            profile.inline.check(&style).unwrap();
        }
        for value in [Value::Px, Value::Ch] {
            assert_eq!(
                Support::ALL.without_value(value).check(&style),
                Err(Unsupported {
                    property,
                    value: Some(value)
                })
            );
        }
        assert_eq!(
            Support::ALL.without_property(property).check(&style),
            Err(Unsupported {
                property,
                value: None
            })
        );
    }
    let spacing = Style::new().letter_spacing(-1.);
    spacing.validate().unwrap();
    assert_eq!(
        Support::ALL
            .without_property(Property::LetterSpacing)
            .check(&spacing),
        Err(Unsupported {
            property: Property::LetterSpacing,
            value: None
        })
    );
    assert_eq!(
        Support::ALL.without_value(Value::Px).check(&spacing),
        Err(Unsupported {
            property: Property::LetterSpacing,
            value: Some(Value::Px)
        })
    );
}

#[test]
fn arithmetic_dimensions_and_signed_offsets_reject_nonfinite_values_before_use() {
    for length in [
        Length::ChPx { ch: -1., px: 4. },
        Length::ChPx { ch: 1., px: -4. },
        Length::ChPx {
            ch: f64::NAN,
            px: 4.,
        },
        Length::ChPx {
            ch: 1.,
            px: f64::INFINITY,
        },
    ] {
        assert!(Style::new().width(length).validate().is_err());
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Style::new().letter_spacing(value).validate().is_err());
        assert!(
            Style::new()
                .left(Length::ChPx { ch: 1., px: value })
                .validate()
                .is_err()
        );
    }
    Style::new()
        .width(Length::ChPx { ch: 0., px: 0. })
        .letter_spacing(0.)
        .validate()
        .unwrap();
}

#[cfg(feature = "css")]
#[test]
fn optional_css_accepts_only_the_finite_two_term_form_and_checks_overridden_originals() {
    use glory_style::Sheet;
    let sheet=Sheet::parse(".code{width:calc(6ch + 144px);left:calc(-1ch - 4px);letter-spacing:12px;letter-spacing:normal;width:20px}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().width,
        Some(Length::ChPx { ch: 6., px: 144. })
    );
    sheet.check(Profile::HTML).unwrap();
    for support in [
        Support::ALL.without_value(Value::Ch),
        Support::ALL.without_value(Value::Px),
        Support::ALL.without_property(Property::LetterSpacing),
    ] {
        assert!(
            sheet
                .check(Profile {
                    inline: support,
                    ..Profile::HTML
                })
                .is_err()
        );
    }
    for source in [
        ".a{width:calc(1ch)}",
        ".a{width:calc(1ch + 2%)}",
        ".a{width:calc(1ch * 2px)}",
        ".a{width:calc(1ch + 2px + 3px)}",
        ".a{max-width:calc(1ch + 2px)}",
        ".a{letter-spacing:1ch}",
    ] {
        assert!(Sheet::parse(source).is_err(), "{source}");
    }
}
