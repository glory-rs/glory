fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::routing::{Bootstrap, Location, RequestHistory, Router};
    use std::{path::PathBuf, rc::Rc};
    let output = PathBuf::from(std::env::args().nth(1).ok_or("pass an output directory")?);
    std::fs::create_dir_all(&output)?;
    let routes = glory_routing_demo::routes();
    let location = Location::parse("/page/1?ssr=1")?;
    let seed = Bootstrap::new(&routes, location.clone(), "demo-v1")?;
    let snapshot = glory::ssr::snapshot_with(move |scope| {
        let router = Router::new(
            scope,
            routes.clone(),
            Rc::new(RequestHistory::new(location)),
        )
        .unwrap();
        glory_routing_demo::page(routes, router.handle())
    })?;
    let assets = "<script type=\"module\">import init,* as demo from '/assets/glory_routing_demo.js';await init();window.demo=demo;window.ready=true;</script>";
    let document = |body: String| {
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><style data-gly-base=\"v1\">{}</style></head><body><div id=\"app\">{body}</div>{assets}</body></html>",
            glory::html::BASE_CSS
        )
    };
    std::fs::write(output.join("csr.html"), document(String::new()))?;
    let html = document(snapshot.html).replace(
        "</body>",
        &format!(
            "<script id=\"route-seed\" type=\"application/x-glory-route\">{}</script></body>",
            seed.to_payload()
        ),
    );
    std::fs::write(output.join("ssr.html"), html)?;
    Ok(())
}
