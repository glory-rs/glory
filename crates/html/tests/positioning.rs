use glory_render::style::{Length, Position, Style};
#[test]
fn fixed_end_insets_serialize_independently_and_clear_to_an_empty_style() {
    assert_eq!(
        glory_html::style(
            &Style::new()
                .position(Position::Fixed)
                .right(Length::Px(12.0))
                .bottom(Length::Percent(5.0))
        ),
        "right:12px;bottom:5%;position:fixed;"
    );
    assert_eq!(
        glory_html::style(&Style::new().right(Length::Auto).bottom(Length::Px(-2.0))),
        "right:auto;bottom:-2px;"
    );
    assert_eq!(glory_html::style(&Style::new()), "");
}
