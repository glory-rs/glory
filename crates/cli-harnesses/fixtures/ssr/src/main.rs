use glory::web::widgets::*;
use glory::{Scope, Widget};

#[derive(Debug)]
struct App;

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div().text("ssr harness").show_in(ctx);
    }
}

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    use glory::web::holders::{SalvoHandler, ServerHolder};
    use salvo::prelude::*;

    let handler = SalvoHandler::new(|config, url| ServerHolder::new(config, url).mount(App)).await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new().push(Router::with_path("<**path>").get(StaticDir::new(["target/site"])));
    let service = salvo::Service::new(router).catcher(salvo::catcher::Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    use glory::web::holders::BrowserHolder;
    BrowserHolder::new().mount(App);
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
