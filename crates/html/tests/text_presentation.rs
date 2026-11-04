use glory_style::{Color, LinearGradient, Style};

#[test]
fn line_clamp_and_text_gradient_lower_to_complete_css_contracts() {
    let css = glory_html::style(&Style::new().line_clamp(2).text_gradient(LinearGradient {
        angle: 90.0,
        stops: vec![Color(255, 0, 0, 255), Color(0, 0, 255, 255)],
    }));
    for expected in [
        "background-image:linear-gradient(90deg,#ff0000ff,#0000ffff)",
        "background-clip:text",
        "-webkit-background-clip:text",
        "color:transparent",
        "display:-webkit-box",
        "-webkit-box-orient:vertical",
        "-webkit-line-clamp:2",
        "overflow:hidden",
    ] {
        assert!(css.contains(expected), "missing {expected} in {css}");
    }
}
