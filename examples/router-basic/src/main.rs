use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use glory::reflow::*;
use glory::routing::aviators::*;
use glory::routing::*;
use glory::web::{event_target_checked, event_target_value, request_animation_frame, window_event_listener};
use glory::web::holders::BrowerHolder;
use glory::web::widgets::*;
use glory::web::{events, NodeRef};
use glory::widgets::*;
use glory::*;
use web_sys::HtmlInputElement;

use app::*;

pub fn main() {
    BrowerHolder::new().enable(BrowserAviator::new(route(), catch())).mount(App::new());
}
