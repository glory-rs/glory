#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory_apace::{Raster, Renderer};
    use glory_render::TextKind;
    use glory_ui::{App, button, check, input, list, text, view};
    use std::{fs::File, io::BufWriter, path::Path};

    fn save(raster: &Raster, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let frame = raster.frame().expect("drawn frame");
        let mut rgba = raster.pixels().to_vec();
        // PNG stores straight alpha; the CPU backbuffer is premultiplied RGBA8.
        for pixel in rgba.as_chunks_mut::<4>().0 {
            let a = u32::from(pixel[3]);
            if a > 0 && a < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        let mut encoder = png::Encoder::new(
            BufWriter::new(File::create(path)?),
            frame.width,
            frame.height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba)?;
        writer.finish()?;
        Ok(())
    }
    let path = std::env::args_os()
        .nth(1)
        .ok_or("pass an output PNG path on the D: build volume")?;
    let path = Path::new(&path);
    let mut output = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        output = Some(host.clone());
        host
    });
    let host = output.expect("renderer");
    host.set_viewport(720, 480, 1.0)?;
    host.set_css(
        r#"
        html,body{height:100%}
        .app{width:100%;height:100%;padding:24px;background:#101827;color:#edf3ff;font-family:sans-serif;display:flex;flex-direction:column;gap:14px}
        .title{font-size:28px;line-height:36px}
        .sub{font-size:14px;line-height:20px;color:#a6b7d6}
        .controls{display:flex;align-items:center;gap:12px;height:42px}
        button{border:0;border-radius:8px;padding:10px 18px;background:#3559e0;color:white}
        input[type=text]{width:230px;height:38px;padding:8px 10px;background:#202e49;border:1px solid #405374;color:#edf3ff;border-radius:6px}
        .check{display:flex;gap:8px;align-items:center}
        .check input{color:#3559e0;accent-color:#3559e0}
        .counter{font-size:18px;line-height:24px}
        .list-shell{padding:10px;border:1px solid #405374;border-radius:10px;background:#142039}
        .entry{height:28px;display:flex;align-items:center;padding:0 10px;color:#dbe8ff;background:#1b2943}
        .entry.odd{background:#20314c}
    "#,
    );
    let count = app.scope().cage(0u64);
    let position = app.scope().cage(0.0);
    app.mount_with(|| {
        view().class("app").fill((
            text()
                .kind(TextKind::Heading(1))
                .class("title")
                .content("Glory on Apace"),
            text()
                .class("sub")
                .content("100 million entries. A small, reusable window of rows."),
            view().class("controls").fill((
                button()
                    .label("Add")
                    .on(glory_ui::events::activate, move |_| {
                        count.revise(|value| *value += 1)
                    }),
                input().value("Search entries"),
                check().class("check").label("Notifications").checked(true),
            )),
            text()
                .class("counter")
                .content(count.map(|value| format!("Count: {value}"))),
            view().class("list-shell").fill(
                list(
                    100_000_000usize,
                    |index| index,
                    |_, key| {
                        view()
                            .class(if key % 2 == 0 { "entry" } else { "entry odd" })
                            .fill(text().content(format!("Entry {key:08}")))
                    },
                )
                .height(196.0)
                .measure(28.0)
                .measure_limit(64)
                .overscan(2)
                .position(position),
            ),
        ))
    })?;
    for _ in 0..4 {
        app.pump()?;
    }
    let mut raster = Raster::new();
    raster.draw(&host)?;
    save(&raster, path)?;
    count.revise(|value| *value = 3);
    position.revise(|value| *value = 3_000_000_000.0);
    app.flush()?;
    for _ in 0..4 {
        app.pump()?;
    }
    raster.draw(&host)?;
    let end = path.with_file_name(format!(
        "{}-end.png",
        path.file_stem()
            .and_then(|value| value.to_str())
            .ok_or("UTF-8 output stem required")?
    ));
    save(&raster, &end)?;
    println!(
        "Saved {} and {}; engine_nodes={}, buffer_bytes={}",
        path.display(),
        end.display(),
        host.stats().engine_nodes,
        raster.pixels().len()
    );
    app.clear()?;
    app.pump()?;
    raster.draw(&host)?;
    assert!(raster.pixels().iter().all(|byte| *byte == 0));
    raster.clear();
    assert_eq!(raster.buffer_capacity(), 0);
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
