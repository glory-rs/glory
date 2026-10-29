#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]

use glory_apace::{Raster, Renderer};
use glory_html::Part;
use glory_render::{Change, Patch, Purpose};
use glory_ui::{App, input};

fn fixture(purpose: Purpose, value: &str) -> (App, Renderer) {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        renderer.set_viewport(320, 60, 1.).unwrap();
        renderer.set_css("body{margin:0;background:#fff}input{appearance:none;width:240px;height:32px;padding:0;border:0;background:#fff;color:#000;font-family:monospace;font-size:18px}");
        output = Some(renderer.clone());
        renderer
    });
    app.mount(input().purpose(purpose).value(value.to_owned()))
        .unwrap();
    assert!(!app.settle(8).unwrap().pending);
    (app, output.unwrap())
}
fn pixels(purpose: Purpose, value: &str) -> Vec<u8> {
    let (_app, renderer) = fixture(purpose, value);
    let mut raster = Raster::new();
    assert!(raster.draw(&renderer).unwrap().rendered);
    raster.pixels().to_vec()
}

#[test]
fn actual_password_pixels_equal_bullets_and_hide_widths_scripts_combining_and_emoji() {
    let expected = pixels(Purpose::Text, "••••••••");
    for value in [
        "Wiim123!".into(),
        "xxxxxxxx".into(),
        "中文中文中文中文".into(),
        "e\u{301}".repeat(8),
        "👨‍👩‍👧‍👦".repeat(8),
    ] {
        assert_eq!(
            pixels(Purpose::Password, &value),
            expected,
            "password shape leaked: {value}"
        );
    }
    assert_ne!(
        pixels(Purpose::Text, "Wiim123!"),
        pixels(Purpose::Text, "xxxxxxxx"),
        "positive text control"
    );
    assert_ne!(
        pixels(Purpose::Password, "Wiim123!"),
        pixels(Purpose::Text, "Wiim123!")
    );
    assert_ne!(
        pixels(Purpose::Password, "x"),
        expected,
        "mask must actually paint"
    );
}

#[test]
fn incremental_password_reveal_and_controlled_unicode_updates_preserve_identity_and_raw_values() {
    let (app, renderer) = fixture(Purpose::Text, "Wiim123!");
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    let writes = renderer.stats().property_writes;
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    let plain = raster.pixels().to_vec();
    app.scope()
        .patch(id, Patch::Purpose(Change::Set(Purpose::Password)))
        .unwrap();
    app.settle(8).unwrap();
    raster.draw(&renderer).unwrap();
    let masked = raster.pixels().to_vec();
    assert_ne!(masked, plain);
    assert_eq!(renderer.stats().property_writes, writes);
    let value = "e\u{301}".repeat(8);
    app.scope()
        .patch(
            id,
            Patch::Value {
                value: Change::Set(value.clone()),
                revision: 2,
            },
        )
        .unwrap();
    app.settle(8).unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(raster.pixels(), masked);
    renderer.inspect(|doc| {
        let input = doc
            .get_node(physical)
            .unwrap()
            .element_data()
            .unwrap()
            .text_input_data()
            .unwrap();
        assert_eq!(input.editor.raw_text(), value);
        assert_eq!(input.display_editor().raw_text(), "••••••••");
    });
    app.scope()
        .patch(id, Patch::Purpose(Change::Clear))
        .unwrap();
    app.settle(8).unwrap();
    raster.draw(&renderer).unwrap();
    assert_ne!(raster.pixels(), masked);
    assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
    app.clear().unwrap();
    assert!(raster.draw(&renderer).unwrap().rendered);
    assert_eq!(renderer.stats().nodes, 0);
}
