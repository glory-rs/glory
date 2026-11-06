use glory_style::{Length, Style};

#[test]
fn mixed_font_pixel_lengths_and_signed_spacing_lower_without_shorthands() {
    let css = glory_html::style(
        &Style::new()
            .width(Length::ChPx { ch: 6., px: 144. })
            .left(Length::ChPx { ch: -1., px: -12. })
            .letter_spacing(24.),
    );
    assert!(css.contains("width:calc(6ch + 144px)"), "{css}");
    assert!(css.contains("left:calc(-1ch - 12px)"), "{css}");
    assert!(css.contains("letter-spacing:24px"), "{css}");
    assert!(!css.contains("font:"));
    assert!(glory_html::style(&Style::new().letter_spacing(0.)).contains("letter-spacing:0px"));
    assert!(glory_html::style(&Style::new().letter_spacing(-1.)).contains("letter-spacing:-1px"));
    assert!(!glory_html::style(&Style::new()).contains("letter-spacing"));
}
