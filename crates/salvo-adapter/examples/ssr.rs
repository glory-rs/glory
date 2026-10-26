//! Run: cargo run -p glory-salvo --example ssr --features stream
//! Visit /plain and /stream. The latter updates twice before its End handoff.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::time::Duration;

    use glory::prelude::*;
    use glory_salvo::{StreamError, StreamPage, StreamRenderer, document_response, ssr};
    use salvo::prelude::*;

    const CSP: &str =
        "default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; base-uri 'none'";

    fn document(title: &str) -> ssr::Document {
        ssr::Document {
            title: title.into(),
            lang: Some("en".into()),
            ..Default::default()
        }
    }

    #[handler]
    async fn plain(response: &mut Response) {
        *response = document_response(
            view().fill((
                text()
                    .kind(TextKind::Heading(1))
                    .content("Glory + Salvo SSR"),
                text().content("This page was rendered synchronously with the semantic UI."),
                link().target("/stream").label("Open progressive SSR"),
            )),
            &document("Synchronous SSR"),
        )
        .unwrap();
        response
            .add_header("content-security-policy", CSP, true)
            .unwrap();
    }

    struct Progressive(StreamRenderer);
    #[handler]
    impl Progressive {
        async fn handle(&self, response: &mut Response) {
            let page = StreamPage {
                document: document("Progressive SSR"),
                bootstrap: ssr::stream::Bootstrap {
                    root_id: "app".into(),
                    data_id: "request-resources".into(),
                    script_url: "/stream.js".into(),
                    nonce: None,
                },
            };
            // Everything local (Resources, Cage/Bond, widget and futures) is
            // constructed on the admitted worker. Only Send request data belongs
            // in this closure's captures. Tokio I/O/timers are available there.
            let result = self
                .0
                .render(page, || {
                    ssr::stream_with(
                        Resources::default(),
                        Default::default(),
                        |scope, resources| {
                            let children = [("fast", 350), ("slow", 1200)]
                                .into_iter()
                                .map(|(name, delay)| {
                                    let value = resources
                                        .bind(
                                            scope.owner(),
                                            name,
                                            "v1",
                                            || 1_u64,
                                            move |_| async move {
                                                tokio::time::sleep(Duration::from_millis(delay))
                                                    .await;
                                                Ok::<_, String>(format!(
                                                    "{name} result arrived after {delay} ms"
                                                ))
                                            },
                                        )
                                        .unwrap();
                                    text().content(value.map(move |state| match state {
                                        ResourceState::Ready { value, .. } => value.to_string(),
                                        ResourceState::Failed { error, .. } => error.to_string(),
                                        _ => format!("Waiting for {name}…"),
                                    }))
                                })
                                .collect::<Vec<_>>();
                            view().fill((
                                text().kind(TextKind::Heading(1)).content("Progressive SSR"),
                                text().content(
                                    "The shell is visible while these two resources load:",
                                ),
                                view().fill(children),
                                link().target("/plain").label("Back to synchronous SSR"),
                            ))
                        },
                    )
                })
                .await;
            match result {
                Ok(rendered) => {
                    *response = rendered;
                    response
                        .add_header("content-security-policy", CSP, true)
                        .unwrap();
                }
                Err(error) => {
                    let status = if matches!(error, StreamError::Busy) {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    };
                    eprintln!("SSR startup failed: {error}");
                    response.status_code(status);
                    response.render(Text::Plain("SSR is unavailable"));
                }
            }
        }
    }

    #[handler]
    async fn stream_script(response: &mut Response) {
        response
            .add_header("content-type", "text/javascript; charset=utf-8", true)
            .unwrap();
        response.write_body(glory_web::STREAM_SCRIPT).unwrap();
    }

    #[tokio::main]
    pub async fn run() {
        // Share this admission limit across routes; overload returns Busy rather
        // than queuing unbounded requests or creating unbounded native threads.
        let renderer = StreamRenderer::new(32).unwrap();
        let router = Router::new()
            .get(plain)
            .push(Router::with_path("plain").get(plain))
            .push(Router::with_path("stream").get(Progressive(renderer)))
            .push(Router::with_path("stream.js").get(stream_script));
        let address = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "127.0.0.1:5800".into());
        println!("Glory SSR: http://{address}/plain and http://{address}/stream");
        let acceptor = TcpListener::new(address).bind().await;
        Server::new(acceptor).serve(router).await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::run();
}
#[cfg(target_arch = "wasm32")]
fn main() {}
