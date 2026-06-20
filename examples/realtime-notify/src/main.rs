mod app;
mod notify;
#[cfg(feature = "web-ssr")]
mod server_ws;

use app::App;
use glory::Holder;

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use glory::web::holders::{SalvoHandler, ServerHolder};
    use salvo::catcher::Catcher;
    use salvo::prelude::*;

    let handler = SalvoHandler::new(|config, url| ServerHolder::new(config, url).mount(App::new())).await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(Router::with_path(notify::NOTIFY_PATH.trim_start_matches('/')).get(server_ws::notify_handler))
        .push(glory::serverfn::salvo_mount::router())
        .push(Router::with_path("<**path>").get(StaticDir::new("target/site")));
    let service = salvo::Service::new(router).catcher(Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    use glory::web::holders::BrowserHolder;
    BrowserHolder::new().mount(App::new());
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
