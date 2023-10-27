use glory::routing::aviators::*;
use glory::web::holders::*;
use glory::*;

#[macro_use]
extern crate cfg_if;

mod views;
use views::App;

#[cfg(feature = "web-ssr")]
pub mod post;

pub mod models;

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use salvo::catcher::Catcher;
    use salvo::prelude::*;

    use crate::models::*;
    use app::*;

    let handler = SalvoHandler::new(|config, url| ServerHolder::new(config, url).enable(ServerAviator::new(route(), catch())).mount(App)).await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(route().make_salvo_router(handler.clone()))
        .push(Router::with_path("<**path>").get(StaticDir::new(["target/site", "ssr-modes-salvo/target/site"])));
    println!("{:#?}", router);
    let service = salvo::Service::new(router).catcher(Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    use app::*;

    BrowerHolder::new().enable(BrowserAviator::new(route(), catch())).mount(App);
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
