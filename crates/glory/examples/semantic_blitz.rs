#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::prelude::*;
    use glory::render::{Query, QueryValue};
    use std::{cell::RefCell, rc::Rc};

    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = glory::blitz::Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.expect("renderer");
    renderer.set_css(".panel { display:flex; gap:12px; padding:8px; width:320px; }");
    let count = app.scope().cage(0_u64);
    app.mount_with(|| {
        view().class("panel").fill((
            text().content(count.map(|value| format!("Count: {value}"))),
            button()
                .label("Add")
                .on(activate, move |_| count.revise(|value| *value += 1)),
        ))
    })?;
    // No window or input driver is created by this headless example.
    count.revise(|value| *value = 1);
    let root = app.tree().roots()[0];
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(Query::Bounds(root), move |value| {
        *reply.borrow_mut() = Some(value)
    })?;
    app.pump()?;
    if let Some(Ok(QueryValue::Bounds(bounds))) = result.borrow_mut().take() {
        println!(
            "Blitz document: {} semantic nodes; panel {} x {} logical pixels",
            renderer.stats().nodes,
            bounds.width,
            bounds.height
        );
    }
    app.clear()?;
    app.pump()?;
    assert_eq!(renderer.stats().nodes, 0);
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
