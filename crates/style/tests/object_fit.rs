use glory_style::{ObjectFit, Profile, Property, Style, Support, Unsupported, Value};

#[test]
fn image_fit_values_have_independent_acceptance_and_valid_defaults() {
    assert_eq!(ObjectFit::default(), ObjectFit::Fill);
    assert_eq!(Style::new().object_fit, None);
    for (fit, value) in [
        (ObjectFit::Fill, Value::FitFill),
        (ObjectFit::Contain, Value::FitContain),
        (ObjectFit::Cover, Value::FitCover),
        (ObjectFit::None, Value::FitNone),
        (ObjectFit::ScaleDown, Value::FitScaleDown),
    ] {
        let style = Style::new().object_fit(fit);
        style.validate().unwrap();
        assert_eq!(
            style.properties().collect::<Vec<_>>(),
            [Property::ObjectFit]
        );
        Profile::HTML.inline.check(&style).unwrap();
        assert_eq!(
            Support::ALL
                .without_property(Property::ObjectFit)
                .check(&style),
            Err(Unsupported {
                property: Property::ObjectFit,
                value: None
            })
        );
        assert_eq!(
            Support::ALL.without_value(value).check(&style),
            Err(Unsupported {
                property: Property::ObjectFit,
                value: Some(value)
            })
        );
    }
    assert_eq!((Profile::HTML.version, Profile::RECORD.version), (9, 9));
}

#[cfg(feature = "css")]
#[test]
fn parser_preserves_original_fit_declarations_and_rejects_nonfinite_vocabulary() {
    use glory_style::Sheet;
    let sheet = Sheet::parse(".a{object-fit:cover;object-fit:fill}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().object_fit,
        Some(ObjectFit::Cover)
    );
    let profile = Profile {
        inline: Support::ALL.without_value(Value::FitCover),
        ..Profile::HTML
    };
    assert!(sheet.check(profile).is_err());
    for (css, fit) in [
        ("fill", ObjectFit::Fill),
        ("contain", ObjectFit::Contain),
        ("cover", ObjectFit::Cover),
        ("none", ObjectFit::None),
        ("scale-down", ObjectFit::ScaleDown),
    ] {
        let sheet = Sheet::parse(&format!(".a{{object-fit:{css}}}")).unwrap();
        assert_eq!(
            sheet.rules()[0].declarations()[0].style().object_fit,
            Some(fit)
        );
    }
    for css in [
        "auto",
        "inherit",
        "50%",
        "contain cover",
        "scale-down fill",
        "",
        "url(a)",
    ] {
        assert!(
            Sheet::parse(&format!(".a{{object-fit:{css}}}")).is_err(),
            "{css}"
        );
    }
}
