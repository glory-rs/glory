use glory::routing::aviators::*;
use glory::web::holders::*;
use glory::*;

mod views;
use views::App;

pub mod models;

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use salvo::catcher::Catcher;
    use salvo::prelude::*;

    let handler = SalvoHandler::new(|config, url| {
        ServerHolder::new(config, url)
            .enable(ServerAviator::new(views::route(), views::catch()))
            .mount(App)
    })
    .await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(views::route().make_salvo_router(handler.clone()))
        .push(Router::with_path("<**path>").get(StaticDir::new("target/site")));
    println!("{:#?}", router);
    let service = salvo::Service::new(router).catcher(Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    BrowerHolder::new().enable(BrowserAviator::new(views::route(), views::catch())).mount(App);
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
