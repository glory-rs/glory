#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use salvo::prelude::*;

    let site_addr = std::env::var("GLORY_SITE_ADDR").unwrap_or_else(|_| "127.0.0.1:8000".to_owned());
    let router = Router::new().get(serve).push(Router::with_path("{**path}").get(serve));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(router).await;
}

#[cfg(feature = "web-ssr")]
#[salvo::handler]
async fn serve(req: &mut salvo::prelude::Request, res: &mut salvo::prelude::Response) {
    use std::path::{Component, Path};

    use salvo::http::StatusCode;

    const INDEX_HTML: &str = r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Glory Counter</title>
    <script type="module">
      import init from "/pkg/counter.js";
      init({ module_or_path: "/pkg/counter.wasm" });
    </script>
  </head>
  <body id="main"></body>
</html>"#;
    const PKG_DIRS: &[&str] = &[
        "target/site/pkg",
        "counter/target/site/pkg",
        "examples/counter/target/site/pkg",
    ];

    let path = req.uri().path();
    if path == "/" || path == "/index.html" {
        res.render(salvo::prelude::Text::Html(INDEX_HTML));
        return;
    }

    let Some(asset) = path.strip_prefix("/pkg/") else {
        res.status_code(StatusCode::NOT_FOUND);
        return;
    };
    if asset.is_empty() || Path::new(asset).components().any(|component| !matches!(component, Component::Normal(_))) {
        res.status_code(StatusCode::NOT_FOUND);
        return;
    }

    for dir in PKG_DIRS {
        let file = Path::new(dir).join(asset);
        if let Ok(bytes) = tokio::fs::read(&file).await {
            let _ = res.add_header("content-type", content_type(asset), true);
            let _ = res.write_body(bytes);
            return;
        }
    }

    res.status_code(StatusCode::NOT_FOUND);
}

#[cfg(feature = "web-ssr")]
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("gz") => "application/gzip",
        Some("br") => "application/octet-stream",
        Some("zst") => "application/zstd",
        _ => "application/octet-stream",
    }
}

#[cfg(not(feature = "web-ssr"))]
fn main() {}
