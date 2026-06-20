//! Static site generation (SSG) — FS7.
//!
//! Two layers live here:
//!
//! 1. A **pure, fully unit-tested core**: [`route_output_path`] maps a route
//!    string to its on-disk `index.html` location, and [`prerender_routes`]
//!    drives an injected `render` closure over a list of routes and writes the
//!    resulting HTML to disk. Neither function knows anything about HTTP, the
//!    app, or cargo — the render closure is the only seam, which keeps the core
//!    trivially testable with a mock renderer.
//!
//! 2. The **CLI production wiring** ([`generate_all`]): it builds the project,
//!    spawns the already-built app server, and uses an HTTP-fetch closure as
//!    the `render` implementation so each configured route is prerendered by
//!    the real SSR pipeline (`ServerHolder::render_string`) running inside the
//!    user's binary, then collected into a static `dist/<name>/` tree.
//!
//! Completion boundary: routes are taken from an explicit list (the
//! `--ssg <route>...` flag). Glory has no build-time access to the user crate's
//! typed route table (it lives in the user binary), so automatic route
//! discovery from a compiled binary is intentionally out of scope; the explicit
//! list + injected render closure is the supported, end-to-end path.

use std::sync::Arc;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::Project;
use crate::ext::anyhow::{Context, Result, anyhow, bail};
use crate::ext::fs;
use crate::logger::GRAY;
use crate::service::serve;

/// Default output folder for the generated static site.
const SSG_DIR: &str = "dist";

/// Map a request route to the file that should hold its prerendered HTML,
/// rooted under `out_dir`.
///
/// Mapping rules:
/// - `/` (or empty / `.`) becomes `index.html`.
/// - `/about` becomes `about/index.html`.
/// - `/blog/post` becomes `blog/post/index.html`.
///
/// The route is normalized first: a leading/trailing slash is irrelevant,
/// duplicate slashes collapse, and `.`/empty segments are dropped. Any segment
/// that would escape `out_dir` — `..` or an embedded drive/absolute marker — is
/// rejected so a hostile or malformed route can never write outside the output
/// directory.
pub fn route_output_path(route: &str, out_dir: impl AsRef<Utf8Path>) -> Result<Utf8PathBuf> {
    let out_dir = out_dir.as_ref();
    let mut path = out_dir.to_path_buf();

    for raw in route.split(['/', '\\']) {
        let segment = raw.trim();
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            bail!("route {route:?} contains a parent-directory `..` segment; refusing to write outside {out_dir}");
        }
        // A Windows drive prefix (`C:`) or any path separator that survived the
        // split would let the segment re-root; reject defensively.
        if segment.contains(':') {
            bail!("route {route:?} segment {segment:?} contains a drive/scheme separator; refusing to write outside {out_dir}");
        }
        path.push(segment);
    }

    Ok(path.join("index.html"))
}

/// Prerender each route to a static HTML file under `out_dir`.
///
/// For every route, `render(route)` is called to produce the HTML body, which
/// is then written to [`route_output_path`]. Parent directories are created as
/// needed. Returns the list of files written, in route order.
///
/// `render` is the only injected dependency: tests pass a deterministic mock,
/// while production passes an HTTP fetch against the running app server (see
/// [`generate_all`]).
pub async fn prerender_routes<F, Fut>(routes: &[String], out_dir: impl AsRef<Utf8Path>, render: F) -> Result<Vec<Utf8PathBuf>>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<String>>,
{
    let out_dir = out_dir.as_ref();
    let mut written = Vec::with_capacity(routes.len());

    for route in routes {
        let dest = route_output_path(route, out_dir)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await.dot()?;
        }
        let html = render(route.clone()).await.with_context(|| format!("render route {route:?}"))?;
        fs::write(&dest, html.as_bytes()).await.dot()?;
        written.push(dest);
    }

    Ok(written)
}

/// Production entry point: build the project, spawn its server, and prerender
/// `routes` by fetching each one over HTTP from the running app, writing the
/// static tree into `dist/<name>/`.
pub async fn generate_all(proj: &Arc<Project>, routes: &[String]) -> Result<()> {
    if routes.is_empty() {
        bail!("`--ssg` requires at least one route to prerender, e.g. `glory build --ssg / --ssg /about`");
    }

    if !super::build::build_proj(proj).await.dot()? {
        bail!("Build failed; nothing to prerender");
    }

    let out_dir = Utf8PathBuf::from(SSG_DIR).join(&proj.name);
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir).await.dot()?;
    }
    fs::create_dir_all(&out_dir).await.dot()?;

    // Spawn the already-built app server; it serves the real SSR pipeline.
    let base_url = proj.site.url();
    let server = serve::spawn(proj).await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build SSG http client")?;

    // Give the freshly spawned child server a moment to bind its socket before
    // the first request; retried inside the fetch closure as well.
    let render = |route: String| {
        let client = client.clone();
        let base_url = base_url.clone();
        async move { fetch_route(&client, &base_url, &route).await }
    };

    let result = prerender_routes(routes, &out_dir, render).await;

    // Always tear the server down, regardless of prerender outcome.
    crate::signal::Interrupt::request_shutdown().await;
    let _ = server.await;

    let written = result?;
    log::info!(
        "Glory prerendered {} route(s) for {} into {}",
        written.len(),
        proj.name,
        GRAY.paint(out_dir.as_str())
    );
    Ok(())
}

/// Fetch a single route's HTML from the running app server, retrying briefly to
/// absorb server startup latency.
async fn fetch_route(client: &reqwest::Client, base_url: &str, route: &str) -> Result<String> {
    let url = join_url(base_url, route);

    let mut last_err = None;
    for attempt in 0..20u32 {
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.with_context(|| format!("read body for {url}"))?;
                if status.is_success() {
                    return Ok(body);
                }
                last_err = Some(anyhow!("route {route:?} returned HTTP {status}"));
                // A non-success status will not fix itself by retrying.
                break;
            }
            Err(err) => {
                last_err = Some(anyhow!("request {url} failed: {err}"));
                tokio::time::sleep(Duration::from_millis(150 * (attempt + 1) as u64)).await;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("failed to fetch route {route:?}")))
}

/// Join a base origin (`http://host:port`) with a route path, tolerating
/// missing/duplicate slashes.
fn join_url(base_url: &str, route: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = route.trim_start_matches('/');
    if path.is_empty() { format!("{base}/") } else { format!("{base}/{path}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_dir::TempDir;

    fn out() -> Utf8PathBuf {
        Utf8PathBuf::from("/tmp/site")
    }

    #[test]
    fn root_route_maps_to_index_html() {
        assert_eq!(route_output_path("/", out()).unwrap(), out().join("index.html"));
        assert_eq!(route_output_path("", out()).unwrap(), out().join("index.html"));
        assert_eq!(route_output_path(".", out()).unwrap(), out().join("index.html"));
    }

    #[test]
    fn single_segment_maps_to_nested_index() {
        assert_eq!(route_output_path("/about", out()).unwrap(), out().join("about").join("index.html"));
        // Trailing slash is irrelevant.
        assert_eq!(route_output_path("/about/", out()).unwrap(), out().join("about").join("index.html"));
        // Leading slash is optional.
        assert_eq!(route_output_path("about", out()).unwrap(), out().join("about").join("index.html"));
    }

    #[test]
    fn nested_route_maps_to_nested_index() {
        let expected = out().join("blog").join("post").join("index.html");
        assert_eq!(route_output_path("/blog/post", out()).unwrap(), expected);
        assert_eq!(route_output_path("blog/post/", out()).unwrap(), expected);
    }

    #[test]
    fn duplicate_and_dot_segments_are_normalized() {
        let expected = out().join("a").join("b").join("index.html");
        assert_eq!(route_output_path("//a///b//", out()).unwrap(), expected);
        assert_eq!(route_output_path("/a/./b", out()).unwrap(), expected);
        // Surrounding whitespace on segments is trimmed.
        assert_eq!(route_output_path("/ a / b ", out()).unwrap(), expected);
    }

    #[test]
    fn parent_directory_segment_is_rejected() {
        assert!(route_output_path("/../escape", out()).is_err());
        assert!(route_output_path("/blog/../../etc/passwd", out()).is_err());
        assert!(route_output_path("..", out()).is_err());
    }

    #[test]
    fn drive_or_scheme_segment_is_rejected() {
        assert!(route_output_path("C:/Windows", out()).is_err());
        // Backslash-separated traversal is also normalized and rejected.
        assert!(route_output_path("..\\..\\secret", out()).is_err());
    }

    #[test]
    fn backslash_route_is_split_like_forward_slash() {
        let expected = out().join("a").join("b").join("index.html");
        assert_eq!(route_output_path("a\\b", out()).unwrap(), expected);
    }

    #[tokio::test]
    async fn prerender_writes_expected_files_with_mock_render() {
        let tmp = TempDir::new().unwrap();
        let out_dir = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
        let routes = vec!["/".to_string(), "/about".to_string(), "/blog/post".to_string()];

        let render = |route: String| async move { Ok(format!("<html><body>route={route}</body></html>")) };

        let written = prerender_routes(&routes, &out_dir, render).await.unwrap();

        assert_eq!(written.len(), 3);
        assert_eq!(written[0], out_dir.join("index.html"));
        assert_eq!(written[1], out_dir.join("about").join("index.html"));
        assert_eq!(written[2], out_dir.join("blog").join("post").join("index.html"));

        for (route, path) in routes.iter().zip(written.iter()) {
            let body = tokio::fs::read_to_string(path).await.unwrap();
            assert_eq!(body, format!("<html><body>route={route}</body></html>"));
        }
        // TempDir cleans the directory tree on drop.
    }

    #[tokio::test]
    async fn prerender_propagates_render_errors_and_traversal() {
        let tmp = TempDir::new().unwrap();
        let out_dir = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

        // Render failure surfaces as an error.
        let failing = vec!["/ok".to_string()];
        let render_err = |_route: String| async move { Err(anyhow!("boom")) };
        assert!(prerender_routes(&failing, &out_dir, render_err).await.is_err());

        // Path traversal is rejected before any render runs.
        let evil = vec!["/../escape".to_string()];
        let render_ok = |_route: String| async move { Ok(String::from("x")) };
        assert!(prerender_routes(&evil, &out_dir, render_ok).await.is_err());
    }

    #[test]
    fn join_url_handles_slashes() {
        assert_eq!(join_url("http://127.0.0.1:3000", "/"), "http://127.0.0.1:3000/");
        assert_eq!(join_url("http://127.0.0.1:3000/", "/about"), "http://127.0.0.1:3000/about");
        assert_eq!(join_url("http://127.0.0.1:3000", "blog/post"), "http://127.0.0.1:3000/blog/post");
        assert_eq!(join_url("http://127.0.0.1:3000", ""), "http://127.0.0.1:3000/");
    }
}
