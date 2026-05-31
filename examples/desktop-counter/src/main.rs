use std::sync::Arc;

use glory_core::renderer::{InsertPosition, Renderer};
use glory_desktop::{RecordingSink, WryRenderer};

fn main() {
    let sink = Arc::new(RecordingSink::default());
    let renderer = WryRenderer::new(sink.clone());
    let root = renderer.create_element("main".into(), false);
    let button = renderer.create_element("button".into(), false);

    renderer.set_text(&button, "Count: 0".into());
    renderer.insert_child(&root, &button, InsertPosition::Tail);

    println!("{:?}", sink.commands());
}
