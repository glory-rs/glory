use glory::routing::aviators::*;
use glory::web::holders::*;
use glory::*;

#[macro_use]
extern crate cfg_if;

mod app;
use app::*;

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

    let handler = SalvoHandler::new(|config, url| {
        ServerHolder::new(config, url)
            .enable(ServerAviator::new(route(), catch()))
            .mount(App::new())
    })
    .await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(
            Router::with_path("api/posts")
                .get(list_posts)
                .push(Router::with_path("<id>").get(get_post)),
        )
        .push(route().make_salvo_router(handler.clone()))
        .push(Router::with_path("<**path>").get(StaticDir::new(["target/site", "ssr-modes-salvo/target/site"])));
    println!("{:#?}", router);
    let service = salvo::Service::new(router).catcher(Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;

    #[handler]
    async fn list_posts() -> Json<Vec<PostMetadata>> {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Json(post::list_posts())
    }

    #[handler]
    async fn get_post(req: &mut Request, res: &mut Response) {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let id = req.param::<usize>("id").unwrap_or_default();
        if let Some(post) = post::get_post(id) {
            res.render(Json(post))
        }
    }
}

#[cfg(feature = "web-csr")]
fn main() {
    BrowerHolder::new().enable(BrowserAviator::new(route(), catch())).mount(App::new());
}
