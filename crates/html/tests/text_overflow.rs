use glory_render::style::{Style, TextOverflow};
#[test]
fn text_overflow_serialization_preserves_explicit_clip_and_ellipsis_independently() {
    assert_eq!(
        glory_html::style(&Style::new().text_overflow(TextOverflow::Clip)),
        "text-overflow:clip;"
    );
    assert_eq!(
        glory_html::style(&Style::new().text_overflow(TextOverflow::Ellipsis)),
        "text-overflow:ellipsis;"
    );
}
