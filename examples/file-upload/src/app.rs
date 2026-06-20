use glory::web::widgets::*;
use glory::{Scope, Widget};

/// A minimal upload page: a native multipart `<form>` that posts to the
/// `/upload` route handled on the server by [`crate::upload`].
#[derive(Debug, Default)]
pub struct App;

impl App {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        main()
            .attr("style", "font-family:system-ui;max-width:40rem;margin:2rem auto")
            .fill(h1().text("File upload"))
            .fill(p().text("Pick a file and submit. The server parses it with serverfn MultipartForm."))
            .fill(
                form()
                    .attr("action", "/upload")
                    .attr("method", "post")
                    .attr("enctype", "multipart/form-data")
                    .fill(
                        p().fill(label().text("Title ").fill(input().attr("type", "text").attr("name", "title"))),
                    )
                    .fill(p().fill(input().attr("type", "file").attr("name", "file").attr("required", true)))
                    .fill(button().attr("type", "submit").text("Upload")),
            )
            .show_in(ctx);
    }
}
