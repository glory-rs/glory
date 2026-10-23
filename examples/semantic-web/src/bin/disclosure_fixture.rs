use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};
fn next(stream: &mut glory::ssr::stream::Stream) -> glory::ssr::stream::Frame {
    let mut future = pin!(stream.next_frame());
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(Some(Ok(frame))) => frame,
        _ => panic!("fixture frame was not ready"),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory")?);
    std::fs::create_dir_all(&directory)?;
    let snapshot =
        glory::ssr::snapshot_with(|scope| glory_semantic_web::disclosure_form::ui(scope, false).0)?;
    let boot = "<script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script>";
    std::fs::write(
        directory.join("disclosure.html"),
        format!(
            "<!doctype html><html><body><div id=\"ssr\">{}</div><div id=\"csr\"></div><div id=\"peer\"></div>{boot}</body></html>",
            snapshot.html
        ),
    )?;
    let mut state = None;
    let mut stream = glory::ssr::stream::stream_with(
        glory::resources::Resources::default(),
        Default::default(),
        |scope, _| {
            let (widget, source) = glory_semantic_web::disclosure_form::ui(scope, false);
            state = Some(source);
            widget
        },
    )?;
    let bootstrap = glory::ssr::stream::Bootstrap {
        root_id: "ssr".into(),
        script_url: "/glory-stream.js".into(),
        data_id: "disclosure-data".into(),
        nonce: None,
    };
    let mut html = next(&mut stream).to_html(&bootstrap)?;
    state
        .as_ref()
        .unwrap()
        .expanded
        .revise(|value| *value = true);
    html.push_str(&next(&mut stream).to_html(&bootstrap)?);
    html.push_str(&next(&mut stream).to_html(&bootstrap)?);
    std::fs::write(
        directory.join("disclosure-stream.html"),
        format!(
            "<!doctype html><html><head>{}</head><body>{html}{boot}</body></html>",
            bootstrap.head_script()?
        ),
    )?;
    std::fs::write(directory.join("glory-stream.js"), glory::web::STREAM_SCRIPT)?;
    Ok(())
}
