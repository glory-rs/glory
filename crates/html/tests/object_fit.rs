use glory_style::{ObjectFit, Style};

#[test]
fn object_fit_serializes_all_values_and_default_reset_independently() {
    for (fit, expected) in [
        (ObjectFit::Fill, "fill"),
        (ObjectFit::Contain, "contain"),
        (ObjectFit::Cover, "cover"),
        (ObjectFit::None, "none"),
        (ObjectFit::ScaleDown, "scale-down"),
    ] {
        assert_eq!(
            glory_html::style(&Style::new().object_fit(fit)),
            format!("object-fit:{expected};")
        );
    }
    assert_eq!(glory_html::style(&Style::new()), "");
}
