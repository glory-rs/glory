use glory_style::{Color, LinearGradient, Style};

#[test]
fn linear_gradient_serializes_as_an_independent_background_image() {
    assert_eq!(
        glory_html::style(&Style::new().background_image(LinearGradient {
            angle: 90.0,
            stops: vec![Color(0, 0, 0, 0), Color(34, 139, 230, 255)],
        })),
        "background-image:linear-gradient(90deg,#00000000,#228be6ff);"
    );
    assert_eq!(glory_html::style(&Style::new()), "");
}
