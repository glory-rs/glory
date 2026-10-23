use glory_render::style::{FlexWrap, Style};
#[test]
fn flex_wrap_serializes_independently_including_explicit_nowrap_reset() {
    assert_eq!(
        glory_html::style(&Style::new().flex_wrap(FlexWrap::Wrap)),
        "flex-wrap:wrap;"
    );
    assert_eq!(
        glory_html::style(&Style::new().flex_wrap(FlexWrap::NoWrap)),
        "flex-wrap:nowrap;"
    );
}
