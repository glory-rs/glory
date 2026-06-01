use glory::routing::aviators::*;
use glory::web::holders::BrowserHolder;
use glory::*;

mod app;
use app::*;

pub fn main() {
    BrowserHolder::new().enable(BrowserAviator::new(route(), catch())).mount(App::new());
}
