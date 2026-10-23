#![cfg(feature = "css")]
use glory_style::{Profile, Sheet};

#[test]
fn lowering_preserves_order_and_specificity_without_selecting_internal_parts() {
    let sheet = Sheet::parse(
        ".a.hot > .b:checked, .c:focus {width:10px;width:20px} .b:disabled{height:30px}",
    )
    .unwrap();
    let css = glory_html::sheet(&sheet, Profile::HTML).unwrap();
    assert!(css.starts_with(":where([data-gly-kind]).a.hot>:where([data-gly-kind]).b[data-gly-checked],:where([data-gly-kind]).c:is(:focus,:where([data-gly-kind=check]):focus-within){width:10px;width:20px;}"));
    assert!(css.ends_with(":where([data-gly-kind]).b[data-gly-disabled]{height:30px;}"));
    assert_eq!(sheet.rules()[0].selectors()[0].specificity(), 4);
    assert!(glory_html::sheet(&sheet, Profile::RECORD).is_err());
}

#[test]
fn escaped_classes_cannot_close_a_style_raw_text_body() {
    let sheet = Sheet::parse(r".\3c \/style\3e {width:10px}").unwrap();
    let css = glory_html::sheet(&sheet, Profile::HTML).unwrap();
    assert!(!css.contains('<'));
    assert!(css.contains(r"\3c "));
}

#[test]
fn face_names_roundtrip_as_css_strings_and_cannot_inject_html_or_declarations() {
    use glory_style::{BorderStyle, Color, Edges, Family, Style, WhiteSpace};
    let names = [
        "serif",
        "Open Sans",
        "汉字🙂",
        "x\";color:red;/*",
        "</style><script>evil</script>",
        "\\\r\n\t\u{7f}0",
    ];
    let style = Style::new().font_family(names.map(|s| Family::Named(s.into())));
    let css = glory_html::style(&style);
    assert!(!css.contains('<'));
    let parsed = Sheet::parse(&format!(".a{{{css}}}")).unwrap();
    assert_eq!(parsed.rules()[0].declarations().len(), 1);
    assert_eq!(parsed.rules()[0].declarations()[0].style(), &style);
    let style = Style::new()
        .border_width(Edges::all(2.0))
        .border_style(BorderStyle::Solid)
        .border_color(Color(1, 2, 3, 255))
        .radius(4.0)
        .opacity(0.5)
        .font_weight(450.5)
        .white_space(WhiteSpace::PreWrap);
    let css = glory_html::style(&style);
    assert!(css.contains("border-width:2px 2px 2px 2px;") && !css.contains("border:"));
    assert!(
        css.contains("border-radius:4px;")
            && css.contains("opacity:0.5;")
            && css.contains("font-weight:450.5;")
            && css.contains("white-space:pre-wrap;")
    );
    assert!(glory_html::style(&Style::new()).is_empty());
}
