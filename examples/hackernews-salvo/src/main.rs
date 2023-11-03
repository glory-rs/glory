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

    use models::*;

    let handler = SalvoHandler::new(|config, url| {
        ServerHolder::new(config, url)
            .enable(ServerAviator::new(views::route(), views::catch()))
            .mount(App)
    })
    .await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(Router::with_path("favicon.ico").get(StaticFile::new("public/favicon.ico")))
        .push(
            Router::with_path("api").push(Router::with_path("/users/<id>").get(show_user)).push(
                Router::with_path("stories")
                    .get(list_stories)
                    .push(Router::with_path("<id:num>").get(show_story)),
            ),
        )
        .push(views::route().make_salvo_router(handler.clone()))
        .push(Router::with_path("<**path>").get(StaticDir::new("target/site")));
    println!("{:#?}", router);
    let service = salvo::Service::new(router).catcher(Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;

    #[handler]
    async fn show_user(req: &mut Request) -> Json<Option<User>> {
        let id = req.param::<&str>("id").unwrap_or_default();
        let api_url = show_user_api_url(id);
        Json(fetch_api::<User>(&api_url).await)
    }
    #[handler]
    async fn list_stories(req: &mut Request) -> Json<Vec<Story>> {
        let cate = req.query::<&str>("cate").unwrap_or_default();
        let page = req.query::<usize>("page").unwrap_or_default();
        let api_url = list_stories_api_url(cate, page);
        Json(fetch_api::<Vec<Story>>(&api_url).await.unwrap_or_default())
    }
    #[handler]
    async fn show_story(req: &mut Request) -> Json<Option<Story>> {
        let id = req.param::<usize>("id").unwrap_or_default();
        let api_url = show_story_api_url(id);
        Json(fetch_api::<Story>(&api_url).await)
    }
}

#[cfg(feature = "web-csr")]
fn main() {
    BrowerHolder::new().enable(BrowserAviator::new(views::route(), views::catch())).mount(App);
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
