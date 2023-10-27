
#[derive(Debug, Clone)]
struct ListPost;
impl Widget for ListPost {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "All posts".to_owned());
        info.description.revise(|mut v| *v = "This page list all posts".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        cfg_if! {
            if #[cfg(feature = "web-csr")] {
                let list = || async {
                    let text = gloo::net::http::Request::get("/api/posts")
                        .send()
                        .await.unwrap_throw().text().await.unwrap_throw();
                        serde_json::from_str::<Vec<PostMetadata>>(&text).unwrap_throw()
                };
            } else {
                let list = || async move {crate::post::list_posts()};
            }
        }
        let loader = Loader::new(list, |posts, ctx| {
            let posts = posts
                .into_iter()
                .map(|p| li().fill(a().attr("href", format!("/{}", p.id)).html(p.title.clone())))
                .collect::<Vec<_>>();

            ul().fill(posts).show_in(ctx);
        })
        .fallback(|ctx| {
            p().html("Loading...").show_in(ctx);
        });

        div().fill(h1().html("All Posts")).fill(loader).show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct ShowPost;
impl Widget for ShowPost {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Post page".to_owned());
        info.description.revise(|mut v| *v = "This is show post page".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("ShowPost::build");
        let post_id: usize = {
            let truck = ctx.truck();
            let locator = truck.obtain::<Locator>().unwrap();
            if let Some(id) = locator.params().get().get("id") {
                id.parse().unwrap_or_default()
            } else {
                0
            }
        };
        cfg_if! {
            if #[cfg(feature = "web-csr")] {
                let post = move || async move{
                    let text = gloo::net::http::Request::get(&format!("/api/posts/{post_id}"))
                        .send()
                        .await.unwrap_throw().text().await.unwrap_throw();
                        Some(serde_json::from_str::<Post>(&text).unwrap_throw())
                };
            } else {
                let post = move || async move {crate::post::get_post(post_id)};
            }
        }
        let info = {
            let truck = ctx.truck();
            truck.obtain::<PageInfo>().unwrap().clone()
        };
        let loader = Loader::new(post, move |post, ctx| {
            if let Some(post) = post {
                info.title.revise(|mut v| *v = post.title.clone());
                info.description.revise(|mut v| *v = post.description.clone());
                article()
                    .fill(h2().html(post.title.clone()))
                    .fill(section().html(post.content.clone()))
                    .show_in(ctx);
            } else {
                article().fill(h2().html("Not found")).show_in(ctx);
            }
        })
        .fallback(|ctx| {
            p().html("Loading...").show_in(ctx);
        });

        div().fill(loader).show_in(ctx);
    }
}

#[derive(Debug, Clone)]
struct NoMatch;
impl Widget for NoMatch {
    fn attach(&mut self, ctx: &mut Scope) {
        let truck = ctx.truck();
        let info = truck.obtain::<PageInfo>().unwrap();
        info.title.revise(|mut v| *v = "Not found page".to_owned());
        info.description.revise(|mut v| *v = "This is not found page".to_owned());
    }
    fn build(&mut self, ctx: &mut Scope) {
        info!("NoMatch::build");
        div()
            .fill(h2().html("Nothing to see here!"))
            .fill(a().attr("href", "/").html("Go to the home page"))
            .show_in(ctx);
    }
}

pub fn route() -> Router {
    Router::new()
        .push(Router::with_path("<id:num>").goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", ShowPost)))
        .goal(|tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", ListPost))
}

pub fn catch() -> impl Handler {
    |tk: Rc<RefCell<Truck>>| tk.insert_stuff("section", NoMatch)
}
