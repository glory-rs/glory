#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::{
        apace::shell::{Shell, WindowApp, WindowAttributes},
        routing::{Location, MemoryHistory, Router},
        ui::{Widget, widget},
    };
    use std::{cell::RefCell, rc::Rc};
    let window = WindowApp::new();
    let handle = Rc::new(RefCell::new(None));
    let capture = handle.clone();
    window.mount(widget(move |scope| {
        let routes = glory_routing_demo::routes();
        let router = Router::new(
            scope,
            routes.clone(),
            Rc::new(MemoryHistory::new(Location::parse("/page/1").unwrap())),
        )
        .unwrap();
        *capture.borrow_mut() = Some(router.handle());
        glory_routing_demo::page(routes, router.handle()).show_in(scope)
    }))?;
    window.attach_router(handle.borrow_mut().take().unwrap())?;
    let mut shell = Shell::new()?;
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Routing"),
    );
    shell.run()?;
    Ok(())
}
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {
    eprintln!("Use an application-owned platform event loop on this target");
}
