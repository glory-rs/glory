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
