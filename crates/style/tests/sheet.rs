#![cfg(feature = "css")]
use glory_style::sheet::{ErrorKind, Limits, Relation, State, Term};
use glory_style::{
    Color, Edges, Family, Length, Limit, Profile, Property, Sheet, Support, WhiteSpace,
};

#[test]
fn selectors_keep_combinators_specificity_escapes_and_declaration_order() {
    let sheet = Sheet::parse(
        "/* comment */ .pane.hot > .row:hover, .a/**/.b .c:FOCUS { width: 20px; WIDTH: 50%; margin: -1px 2px 3px; color:#3a7f; max-width:none }",
    )
    .unwrap();
    let rule = &sheet.rules()[0];
    assert_eq!(rule.selectors().len(), 2);
    assert_eq!(rule.selectors()[0].specificity(), 4);
    assert_eq!(
        rule.selectors()[0].compounds()[1].relation(),
        Some(Relation::Child)
    );
    assert_eq!(
        rule.selectors()[1].compounds()[0].terms(),
        [Term::Class("a".into()), Term::Class("b".into())]
    );
    assert_eq!(
        rule.selectors()[1].compounds()[1].relation(),
        Some(Relation::Descendant)
    );
    assert_eq!(
        rule.selectors()[1].compounds()[1].terms()[1],
        Term::State(State::Focus)
    );
    let decl = rule.declarations();
    assert_eq!(decl.len(), 5);
    assert_eq!(decl[0].style().width, Some(Length::Px(20.0)));
    assert_eq!(decl[1].style().width, Some(Length::Percent(50.0)));
    assert_eq!(
        decl[2].style().margin,
        Some(Edges {
            top: -1.0,
            right: 2.0,
            bottom: 3.0,
            left: 2.0
        })
    );
    assert_eq!(decl[3].style().color, Some(Color(51, 170, 119, 255)));
    assert_eq!(decl[4].style().max_width, Some(Limit::None));
    let escaped = Sheet::parse(r".\70 ane.\3c safe { padding: 0 }").unwrap();
    assert_eq!(
        escaped.rules()[0].selectors()[0].compounds()[0].terms(),
        [Term::Class("pane".into()), Term::Class("<safe".into())]
    );
}

#[test]
fn unsupported_input_never_returns_a_partly_accepted_sheet() {
    for css in [
        ".ok{width:10px}.bad{wibble:1}",
        ".a {width:-1px}",
        ".a {padding:-1px}",
        ".a {flex-grow:-1}",
        ".a{width:1e99px}",
        ".a{font-size:0}",
        ".a{width:10px!important}",
        ".a {width:calc(1px + 2px)}",
        ".a{color:var(--color)}",
        ".a{background-color:url(x)}",
        "@import 'x'; .a{width:1px}",
        "@charset 'utf-8'; .a{width:1px}",
        "<!-- /* comment */ @charset 'utf-8'; .a{width:1px}",
        "@media screen {.a{width:1px}}",
        ".a{--x:1}",
        "div{width:1px}",
        "#a{width:1px}",
        ".a+ .b{width:1px}",
        ".a:nth-child(2){width:1px}",
        ".a:has(.b){width:1px}",
        ".a::before{width:1px}",
        ".a > {width:1px}",
        ".a > > .b{width:1px}",
        ".a, {width:1px}",
        ".a { .b {width:1px} }",
        ".a{width:1px garbage}",
        ".a{max-width:auto}",
        ".a{gap:2%}",
        ".a{height:1rem}",
        ".a{font-family:}",
        ".a{font-family:''}",
        ".a{font-family:serif font}",
        ".a{font-family:inherit}",
        ".a{font-family:ui-rounded}",
        ".a{font-family:serif,}",
        ".a{font-weight:0}",
        ".a{font-weight:1001}",
        ".a{font-weight:bolder}",
        ".a{white-space:break-spaces}",
        ".a{border-width:-1px}",
        ".a{border-style:dotted}",
        ".a{border-radius:1px 2px}",
        ".a{border-radius:1%}",
        ".a{border-radius:-1px}",
        ".a{opacity:1.1}",
        ".a{opacity:-0.1}",
        ".a{opacity:50%}",
    ] {
        assert!(Sheet::parse(css).is_err(), "accepted {css}");
    }
    let err = Sheet::parse(".a {width:1px}\n.b { unknown: 2; }").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(matches!(err.kind,ErrorKind::Property(name) if name=="unknown"));
}

#[test]
fn profile_rejection_uses_each_original_declaration_not_the_final_winner() {
    let sheet = Sheet::parse(".a { position:sticky; position:static }").unwrap();
    let profile = Profile {
        inline: Support::ALL.without_property(Property::Position),
        ..Profile::HTML
    };
    assert!(matches!(
        sheet.check(profile).unwrap_err().kind,
        ErrorKind::Unsupported(_)
    ));
    assert!(sheet.check(Profile::HTML).is_ok());
}

#[test]
fn every_typed_property_has_a_valid_finite_text_representation() {
    let css = ".all{position:relative;top:-2px;left:3%;display:flex;flex-direction:row;width:20px;height:30%;min-width:0;min-height:auto;max-width:none;max-height:100%;gap:2px;padding:1px 2px 3px 4px;margin:-1px;flex-grow:.5;flex-shrink:2;align-items:center;justify-content:space-between;overflow-x:auto;overflow-y:hidden;overflow-anchor:none;color:#123;background-color:transparent;font-size:16px;line-height:20px}";
    let css = css.replace("line-height:20px", "line-height:20px;font-family:system-ui;font-weight:450.5;white-space:pre-wrap;border-width:1px 2px;border-color:#123;border-style:solid;border-radius:4px;opacity:.5");
    let css = css.replace("line-height:20px", "line-height:20px;field-sizing:content");
    let css = css.replace(
        "position:relative",
        "position:relative;right:auto;bottom:4px",
    );
    let css = css.replace(
        "field-sizing:content",
        "field-sizing:content;box-shadow:0 2px 4px #0003;flex-wrap:wrap;text-overflow:ellipsis;grid-template-columns:1fr auto;grid-template-rows:none;grid-column:1 / span 2;grid-row:span 1;aspect-ratio:16 / 9;object-fit:cover;pointer-events:none",
    );
    let sheet = Sheet::parse(&css).unwrap();
    let mut actual: Vec<_> = sheet.rules()[0]
        .declarations()
        .iter()
        .flat_map(|d| d.style().properties())
        .collect();
    let mut expected = Property::ALL.to_vec();
    actual.sort_by_key(|p| p.name());
    expected.sort_by_key(|p| p.name());
    assert_eq!(actual, expected);
    sheet.check(Profile::HTML).unwrap();
}

#[test]
fn field_sizing_checks_each_original_declaration_and_rejects_unknown_values() {
    use glory_style::FieldSizing;
    let sheet = Sheet::parse(".a{field-sizing:content;field-sizing:fixed}").unwrap();
    assert_eq!(
        sheet.rules()[0].declarations()[0].style().field_sizing,
        Some(FieldSizing::Content)
    );
    assert_eq!(
        sheet.rules()[0].declarations()[1].style().field_sizing,
        Some(FieldSizing::Fixed)
    );
    let profile = Profile {
        inline: Support::ALL.without_value(glory_style::Value::FieldContent),
        ..Profile::HTML
    };
    assert!(sheet.check(profile).is_err());
    assert!(Sheet::parse(".a{field-sizing:auto}").is_err());
}

#[test]
fn font_names_remain_distinct_from_generics_and_whitespace_preserves_intent() {
    let sheet = Sheet::parse(
        r#".a{font-family:"serif",Open Sans,MONOSPACE; font-weight:bold; white-space:pre-wrap}"#,
    )
    .unwrap();
    let declarations = sheet.rules()[0].declarations();
    assert_eq!(
        declarations[0].style().font_family,
        Some(vec![
            Family::Named("serif".into()),
            Family::Named("Open Sans".into()),
            Family::Mono
        ])
    );
    assert_eq!(declarations[1].style().font_weight, Some(700.0));
    assert_eq!(
        declarations[2].style().white_space,
        Some(WhiteSpace::PreWrap)
    );
}

#[test]
fn parser_budgets_bound_source_rules_selectors_terms_and_declarations() {
    let css = ".a.b,.c { width:1px; height:2px } .d { padding:0 }";
    let exact = Limits {
        bytes: css.len(),
        rules: 2,
        selectors: 3,
        terms: 4,
        declarations: 3,
    };
    assert!(Sheet::parse_with(css, exact).is_ok());
    for (limits, unit) in [
        (
            Limits {
                bytes: css.len() - 1,
                ..exact
            },
            "bytes",
        ),
        (Limits { rules: 1, ..exact }, "rules"),
        (
            Limits {
                selectors: 2,
                ..exact
            },
            "selectors",
        ),
        (Limits { terms: 3, ..exact }, "terms"),
        (
            Limits {
                declarations: 2,
                ..exact
            },
            "declarations",
        ),
    ] {
        assert_eq!(
            Sheet::parse_with(css, limits).unwrap_err().kind,
            ErrorKind::Limit(unit)
        );
    }
    assert!(
        Sheet::parse_with(
            "",
            Limits {
                bytes: 0,
                rules: 0,
                selectors: 0,
                terms: 0,
                declarations: 0
            }
        )
        .is_ok()
    );
}

#[test]
fn a_single_selector_cannot_evade_matching_depth_budget_with_a_large_sheet_limit() {
    let exact = format!(
        "{}{{width:1px}}",
        std::iter::repeat_n(".a", glory_style::sheet::MAX_SELECTOR_TERMS)
            .collect::<Vec<_>>()
            .join(" > ")
    );
    assert!(Sheet::parse(&exact).is_ok());
    let too_many = exact.replace("{width", ">.a{width");
    assert_eq!(
        Sheet::parse(&too_many).unwrap_err().kind,
        ErrorKind::Limit("selector terms")
    );
}
