use glory_render::style::{BoxShadow, Color, Shadow, Style};
#[test]
fn shadow_serialization_is_explicit_and_preserves_unrelated_declarations() {
    assert_eq!(glory_html::style(&Style::new()), "");
    assert_eq!(
        glory_html::style(&Style::new().box_shadow(BoxShadow::None)),
        "box-shadow:none;"
    );
    let style = Style::new()
        .radius(4.0)
        .box_shadow(BoxShadow::Outset(Shadow {
            x: -2.0,
            y: 3.0,
            blur: 4.0,
            spread: -1.0,
            color: Color(10, 20, 30, 40),
        }));
    let css = glory_html::style(&style);
    assert!(css.contains("border-radius:4px;"));
    assert!(css.contains("box-shadow:-2px 3px 4px -1px #0a141e28;"));
}
