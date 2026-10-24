use glory_render::style::{Display, Style};
#[test]
fn inline_flex_serializes_with_explicit_block_and_flex_resets() {
    for (display, expected) in [
        (Display::InlineFlex, "display:inline-flex;"),
        (Display::Flex, "display:flex;"),
        (Display::Block, "display:block;"),
        (Display::None, "display:none;"),
    ] {
        assert_eq!(glory_html::style(&Style::new().display(display)), expected);
    }
}
