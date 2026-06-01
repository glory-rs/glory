use std::sync::Arc;
use std::{fmt::Display, net::SocketAddr};

use once_cell::sync::Lazy;
use salvo::prelude::*;
use salvo::websocket::{Message, WebSocket};
use serde::Serialize;
use tokio::{net::TcpStream, select, sync::RwLock, task::JoinHandle};

use crate::config::Project;
use crate::ext::sync::wait_for_socket;
use crate::logger::GRAY;
use crate::signal::Interrupt;
use crate::signal::{ReloadSignal, ReloadType};

static SITE_ADDR: Lazy<RwLock<SocketAddr>> = Lazy::new(|| RwLock::new(SocketAddr::new([127, 0, 0, 1].into(), 8000)));
static CSS_LINK: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::default()));

pub async fn spawn(proj: &Arc<Project>) -> JoinHandle<()> {
    let proj = proj.clone();

    let mut site_addr = SITE_ADDR.write().await;
    *site_addr = proj.site.addr;
    if let Some(file) = &proj.style.file {
        let mut css_link = CSS_LINK.write().await;
        // Always use `/` as separator in links
        *css_link = file.site.components().map(|c| c.as_str()).collect::<Vec<_>>().join("/");
    }

    tokio::spawn(async move {
        let _change = ReloadSignal::subscribe();

        let reload_addr = proj.site.reload;

        if TcpStream::connect(&reload_addr).await.is_ok() {
            log::error!("Reload TCP port {reload_addr} already in use. You can set the port in the server integration's RenderOptions reload_port");
            Interrupt::request_shutdown().await;

            return;
        }

        log::debug!("Reload server started {}", GRAY.paint(reload_addr.to_string()));

        let router = Router::with_path("/live_reload").get(live_reload);
        let acceptor = TcpListener::new(reload_addr).bind().await;
        match salvo::Server::new(acceptor).try_serve(router).await {
            Ok(_) => log::debug!("Reload server stopped"),
            Err(e) => log::error!("Reload {e}"),
        }
    })
}

#[handler]
async fn live_reload(req: &mut Request, res: &mut Response) -> Result<(), StatusError> {
    WebSocketUpgrade::new().upgrade(req, res, handle_socket).await
}

async fn handle_socket(mut stream: WebSocket) {
    let mut rx = ReloadSignal::subscribe();
    let mut int = Interrupt::subscribe_any();

    log::trace!("Reload websocket connected");
    tokio::spawn(async move {
        loop {
            select! {
                res = rx.recv() =>{
                    match res {
                        Ok(ReloadType::Full) => {
                            send_and_close(stream, BrowserReloadMessage::Full).await;
                            return
                        }
                        Ok(ReloadType::Style) => {
                            send(&mut stream, BrowserReloadMessage::style().await).await;
                        },
                        Ok(ReloadType::FunctionReloads(data)) => {
                            send(&mut stream, BrowserReloadMessage::functions(data)).await;
                        }
                        Err(e) => log::debug!("Reload recive error {e}")
                    }
                }
                _ = int.recv(), if Interrupt::is_shutdown_requested().await => {
                    log::trace!("Reload websocket closed");
                    return
                },
            }
        }
    });
}

async fn send(stream: &mut WebSocket, msg: BrowserReloadMessage) {
    let site_addr = *SITE_ADDR.read().await;
    if !wait_for_socket("Reload", site_addr).await {
        log::warn!(r#"Reload could not send "{msg}" to websocket"#);
    }

    let text = serde_json::to_string(&msg).unwrap();
    match stream.send(Message::text(text)).await {
        Err(e) => {
            log::debug!("Reload could not send {msg} due to {e}");
        }
        Ok(_) => {
            log::debug!(r#"Reload sent "{msg}" to browser"#);
        }
    }
}

async fn send_and_close(mut stream: WebSocket, msg: BrowserReloadMessage) {
    send(&mut stream, msg).await;
    let _ = stream.close().await;
    log::trace!("Reload websocket closed");
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BrowserReloadMessage {
    Full,
    Style { css_path: String },
    Functions { payload: String },
}

impl BrowserReloadMessage {
    async fn style() -> Self {
        let link = CSS_LINK.read().await.clone();
        if link.is_empty() {
            log::error!("Reload internal error: sending css reload but no css file is set.");
        }
        Self::Style { css_path: link }
    }

    fn functions(data: String) -> Self {
        Self::Functions { payload: data }
    }
}

impl Display for BrowserReloadMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "reload all"),
            Self::Style { css_path } => write!(f, "reload {css_path}"),
            Self::Functions { .. } => write!(f, "reload functions"),
        }
    }
}
