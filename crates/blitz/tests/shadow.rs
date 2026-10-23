#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_blitz::{Raster, Renderer};
use glory_render::style::{BoxShadow, Color, Length, Shadow, Style};
use glory_ui::{App, view};
fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
    let width = raster.frame().unwrap().width as usize;
    &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
}
#[test]
fn typed_shadow_has_real_native_pixels_and_clears_without_replacing_the_box() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(64, 64, 1.0).unwrap();
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let style = app.scope().cage(Style {
        background: Some(Color(255, 255, 255, 255)),
        ..Style::new()
            .width(Length::Px(20.0))
            .height(Length::Px(20.0))
            .box_shadow(BoxShadow::Outset(Shadow {
                x: 8.0,
                y: 8.0,
                blur: 0.0,
                spread: 0.0,
                color: Color(255, 0, 0, 255),
            }))
    });
    app.mount(view().style(style)).unwrap();
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 24, 24), [255, 0, 0, 255]);
    assert_eq!(pixel(&raster, 4, 4), [255, 255, 255, 255]);
    style.revise(|s| s.box_shadow = Some(BoxShadow::None));
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 24, 24), [0, 0, 0, 0]);
    assert_eq!(
        renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        physical
    );
    style.revise(|s| {
        s.box_shadow = Some(BoxShadow::Outset(Shadow {
            x: 8.0,
            y: 8.0,
            blur: 4.0,
            spread: 2.0,
            color: Color(255, 0, 0, 255),
        }))
    });
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert!(pixel(&raster, 27, 27)[3] > 0, "blur/spread shadow absent");
}
