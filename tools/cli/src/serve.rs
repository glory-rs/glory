use crate::Result;
use crate::args::Args;
use crate::build;
use crate::project::{Platform, Project};
use notify::{RecursiveMode, Watcher};
use serde_json::json;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const RELOAD: &str = "let revision=Number(new URL(import.meta.url).searchParams.get('revision'));let overlay;setInterval(async()=>{try{const r=await fetch('/__glory_dev/state',{cache:'no-store'});const s=await r.json();if(s.error){if(!overlay){overlay=document.createElement('pre');overlay.setAttribute('role','alert');document.body.prepend(overlay);}overlay.textContent=s.error;}else{overlay?.remove();overlay=null;}if(revision!==s.revision){location.reload();}revision=s.revision;}catch{}},750);";

pub fn serve(mut project: Project, mut args: Args) -> Result<()> {
    if project.platform != Platform::Web {
        return Err("serve currently supports Web; use run for desktop/server".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Release)).map_err(|e| e.to_string())?;
    args.cancel = Some(stop.clone());
    project.cancel = Some(stop.clone());
    let built = build::build(&project, &args)?;
    let directory = built.directory.ok_or("Web build has no output directory")?;
    let listener =
        TcpListener::bind(("127.0.0.1", args.port.or(project.host.port).unwrap_or(8080)))
            .map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let changed = Arc::new(AtomicBool::new(false));
    let mut watcher = if args.watch {
        Some(watch(&project, changed.clone())?)
    } else {
        None
    };
    let mut revision = 1_u64;
    let mut error = None;
    let mut last_event = None;
    println!(
        "Glory: http://{} (Ctrl+C to stop)",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    while !stop.load(Ordering::Acquire) {
        for _ in 0..32 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if let Err(error) = respond(
                        &mut stream,
                        &directory,
                        revision,
                        error.as_deref(),
                        args.watch,
                    ) {
                        eprintln!("HTTP: {error}");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.to_string()),
            }
        }
        if changed.swap(false, Ordering::AcqRel) {
            last_event = Some(Instant::now());
        }
        if last_event.is_some_and(|time| time.elapsed() >= Duration::from_millis(200)) {
            last_event = None;
            println!("Sources changed; rebuilding...");
            match Project::load(&project.manifest, &args).and_then(|current| {
                build::build(&current, &args)?;
                Ok(current)
            }) {
                Ok(current) => {
                    project = current;
                    revision = revision
                        .checked_add(1)
                        .ok_or("development revision exhausted")?;
                    error = None;
                    watcher = Some(watch(&project, changed.clone())?);
                    println!("Rebuild complete (revision {revision})");
                }
                Err(message) => {
                    eprintln!("{message}");
                    error = Some(message.chars().take(32_768).collect::<String>());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    drop(watcher);
    Ok(())
}

fn watch(project: &Project, changed: Arc<AtomicBool>) -> Result<notify::RecommendedWatcher> {
    let build_root = project.paths.root.clone();
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<notify::Event>| match result {
            Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                if event.paths.iter().any(|path| {
                    !path.starts_with(&build_root)
                        && !path.components().any(|part| {
                            matches!(
                                part.as_os_str().to_str(),
                                Some(".git" | "target" | "node_modules")
                            )
                        })
                }) {
                    changed.store(true, Ordering::Release);
                }
            }
            Err(error) => eprintln!("watch: {error}"),
            _ => {}
        })
        .map_err(|e| e.to_string())?;
    let mut roots = project.watch_roots.clone();
    roots.sort();
    roots.dedup();
    let mut installed = Vec::<PathBuf>::new();
    for root in roots {
        if installed.iter().any(|parent| root.starts_with(parent)) {
            continue;
        }
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| format!("watch {}: {e}", root.display()))?;
        installed.push(root);
    }
    Ok(watcher)
}

fn respond(
    stream: &mut TcpStream,
    root: &Path,
    revision: u64,
    error: Option<&str>,
    reload: bool,
) -> Result<()> {
    // Windows accepts can inherit the listener's nonblocking mode. Request
    // reads use deadlines, and must wait for headers rather than reset clients.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let count = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..count]);
        if request.len() > 16_384 {
            return reply(
                stream,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"header too large",
                false,
            );
        }
    }
    let request = match std::str::from_utf8(&request) {
        Ok(request) => request,
        Err(_) => {
            return reply(
                stream,
                "400 Bad Request",
                "text/plain",
                b"invalid request",
                false,
            );
        }
    };
    let mut words = request.lines().next().unwrap_or("").split_whitespace();
    let method = words.next().unwrap_or("");
    let url = words.next().unwrap_or("");
    let head = method == "HEAD";
    if !matches!(method, "GET" | "HEAD") {
        return reply(
            stream,
            "405 Method Not Allowed",
            "text/plain",
            b"GET or HEAD required",
            false,
        );
    }
    let path = url.split('?').next().unwrap_or("");
    if path == "/__glory_dev/state" {
        let body = serde_json::to_vec(&json!({ "revision": revision, "error": error }))
            .map_err(|e| e.to_string())?;
        return reply(stream, "200 OK", "application/json", &body, head);
    }
    if path == "/__glory_dev/reload.js" {
        return reply(stream, "200 OK", "text/javascript", RELOAD.as_bytes(), head);
    }
    let relative = match decode_path(path) {
        Ok(path) => path,
        Err(_) => {
            return reply(
                stream,
                "400 Bad Request",
                "text/plain",
                b"invalid path",
                head,
            );
        }
    };
    let file = match root.join(&relative).canonicalize() {
        Ok(file)
            if file.starts_with(root.canonicalize().map_err(|e| e.to_string())?)
                && file.is_file() =>
        {
            file
        }
        _ => return reply(stream, "404 Not Found", "text/plain", b"not found", head),
    };
    let mut body = std::fs::read(&file).map_err(|e| e.to_string())?;
    let kind = match file
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    };
    if reload
        && file
            .extension()
            .is_some_and(|extension| extension == "html")
        && let Ok(html) = std::str::from_utf8(&body)
    {
        let script = format!(
            "<script type=\"module\" src=\"/__glory_dev/reload.js?revision={revision}\"></script>"
        );
        let position = html.rfind("</body>").unwrap_or(html.len());
        body = format!("{}{script}{}", &html[..position], &html[position..]).into_bytes();
    }
    reply(stream, "200 OK", kind, &body, head)
}

fn reply(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8], head: bool) -> Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    if !head {
        stream.write_all(body).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn decode_path(path: &str) -> Result<PathBuf> {
    if !path.starts_with('/') {
        return Err("expected origin-relative path".into());
    }
    let mut decoded = Vec::new();
    let mut bytes = path.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|byte| (byte as char).to_digit(16))
                .ok_or("invalid percent escape")?;
            let low = bytes
                .next()
                .and_then(|byte| (byte as char).to_digit(16))
                .ok_or("invalid percent escape")?;
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|e| e.to_string())?;
    if decoded.contains(['\\', ':', '\0']) {
        return Err("invalid path character".into());
    }
    let relative = PathBuf::from(decoded.trim_start_matches('/'));
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("path escaped the site".into());
    }
    Ok(if relative.as_os_str().is_empty() {
        PathBuf::from("index.html")
    } else {
        relative
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_traversal_and_windows_paths_cannot_escape_the_site() {
        for path in [
            "/../secret",
            "/%2e%2e/secret",
            "/a/%2E%2E/secret",
            "/D:/secret",
            "/a%5csecret",
            "/%00",
            "/%",
        ] {
            assert!(decode_path(path).is_err(), "{path}");
        }
        assert_eq!(
            decode_path("/some%20file.js").unwrap(),
            PathBuf::from("some file.js")
        );
    }

    fn request(root: &Path, request: &[u8], reload: bool) -> Vec<u8> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let root = root.to_path_buf();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(true).unwrap();
            respond(&mut stream, &root, 7, Some("<compiler error>"), reload).unwrap();
        });
        let mut client = TcpStream::connect(address).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        client.write_all(request).unwrap();
        let mut bytes = Vec::new();
        client.read_to_end(&mut bytes).unwrap();
        worker.join().unwrap();
        bytes
    }

    #[test]
    fn real_http_serves_wasm_head_and_development_state_without_mutating_release_files() {
        let root = crate::test_support::Scratch::new();
        let html = "<html><body>app</body></html>";
        root.write("index.html", html);
        root.write("app.wasm", "wasm-bytes");
        let response = request(&root.0, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n", true);
        assert!(
            String::from_utf8(response)
                .unwrap()
                .contains("/__glory_dev/reload.js")
        );
        assert_eq!(
            std::fs::read_to_string(root.0.join("index.html")).unwrap(),
            html
        );
        let response = request(
            &root.0,
            b"HEAD /app.wasm HTTP/1.1\r\nHost: localhost\r\n\r\n",
            false,
        );
        let response = String::from_utf8(response).unwrap();
        assert!(response.contains("Content-Type: application/wasm"));
        assert!(response.ends_with("\r\n\r\n"));
        let response = request(
            &root.0,
            b"GET /__glory_dev/state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            true,
        );
        let response = String::from_utf8(response).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(json["revision"], 7);
        assert_eq!(json["error"], "<compiler error>");
    }
}
