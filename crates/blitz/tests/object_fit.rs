#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_blitz::assets::{Bytes, NetHandler, NetProvider, Request};
use glory_blitz::{Config, Raster, Renderer};
use glory_render::style::{Length, ObjectFit, Style};
use glory_ui::{App, image};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Provider(Mutex<Vec<(String, Box<dyn NetHandler>)>>);
impl NetProvider for Provider {
    fn fetch(&self, _: usize, request: Request, handler: Box<dyn NetHandler>) {
        self.0
            .lock()
            .unwrap()
            .push((request.url.to_string(), handler));
    }
}
fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
    let width = raster.frame().unwrap().width as usize;
    &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
}
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 40, 20);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let mut data = Vec::new();
        for _ in 0..20 {
            for x in 0..40 {
                data.extend(if x < 5 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                });
            }
        }
        writer.write_image_data(&data).unwrap();
    }
    bytes
}
#[test]
fn typed_image_fit_uses_real_native_image_pixels_for_all_values_and_reset() {
    let provider = Arc::new(Provider::default());
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                provider: Some(provider.clone()),
                ..Default::default()
            },
        );
        host.set_viewport(100, 100, 1.0).unwrap();
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let style = app.scope().cage(
        Style::new()
            .width(Length::Px(80.0))
            .height(Length::Px(80.0))
            .object_fit(ObjectFit::Fill),
    );
    app.mount(image().source("photo.png").alt("Pattern").style(style))
        .unwrap();
    app.pump().unwrap();
    let jobs = std::mem::take(&mut *provider.0.lock().unwrap());
    assert_eq!(jobs.len(), 1);
    for (url, handler) in jobs {
        handler.bytes(url, Bytes::from(png()));
    }
    for _ in 0..3 {
        app.pump().unwrap();
    }
    let id = app.tree().roots()[0];
    let engine = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let mut raster = Raster::new();
    let clear = [0, 0, 0, 0];
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    for fit in [
        ObjectFit::Fill,
        ObjectFit::Contain,
        ObjectFit::Cover,
        ObjectFit::None,
        ObjectFit::ScaleDown,
    ] {
        style.revise(|value| value.object_fit = Some(fit));
        app.flush().unwrap();
        raster.draw(&renderer).unwrap();
        match fit {
            ObjectFit::Fill => {
                assert_eq!(pixel(&raster, 2, 2), blue);
                assert_eq!(pixel(&raster, 40, 2), red);
            }
            ObjectFit::Contain => {
                assert_eq!(pixel(&raster, 2, 2), clear);
                assert_eq!(pixel(&raster, 2, 25), blue);
                assert_eq!(pixel(&raster, 40, 25), red);
            }
            ObjectFit::Cover => {
                assert_eq!(pixel(&raster, 2, 2), red);
                assert_eq!(pixel(&raster, 75, 75), red);
            }
            ObjectFit::None | ObjectFit::ScaleDown => {
                assert_eq!(pixel(&raster, 2, 2), clear);
                assert_eq!(pixel(&raster, 21, 31), blue);
                assert_eq!(pixel(&raster, 40, 40), red);
            }
        }
        assert_eq!(
            renderer.engine_id(id, glory_html::Part::Main).unwrap(),
            engine
        );
    }
    style.revise(|value| {
        value.width = Some(Length::Px(20.0));
        value.height = Some(Length::Px(20.0));
        value.object_fit = Some(ObjectFit::ScaleDown);
    });
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 1, 1), clear);
    assert_eq!(pixel(&raster, 1, 7), blue);
    assert_eq!(pixel(&raster, 10, 7), red);
    style.revise(|value| value.object_fit = None);
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 1, 1), blue);
    assert_eq!(pixel(&raster, 10, 1), red);
}
