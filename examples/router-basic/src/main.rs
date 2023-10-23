use glory::routing::aviators::*;
use glory::web::holders::BrowerHolder;
use glory::*;

mod app;
use app::*;

pub fn main() {
    BrowerHolder::new().enable(BrowserAviator::new(route(), catch())).mount(App::new());
}
