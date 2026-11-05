//! Explicit HTTP(S) reader windows, isolated from the trusted UI WebView.
//! No IPC, application protocols, bootstrap or application credentials are
//! installed in page windows. Native callbacks enqueue bounded observations.
use super::{Message, ShellError};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    task::Waker,
};
use tao::{
    event::WindowEvent,
    event_loop::EventLoopWindowTarget,
    window::{Window, WindowBuilder, WindowId},
};
use wry::{WebView, WebViewBuilder};
const LIMIT: usize = 16;
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub key: String,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub open: bool,
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Back,
    Forward,
    Reload,
    Show,
    Hide,
    Close,
}
enum Command {
    Open {
        key: String,
        url: String,
        title: String,
        generation: u64,
    },
    Action(String, Action),
}
#[derive(Default)]
struct Shared {
    commands: VecDeque<Command>,
    observations: VecDeque<(String, u64, Observation)>,
    entries: BTreeMap<String, (u64, Snapshot)>,
    next: u64,
    waker: Option<Waker>,
    closed: bool,
}
#[derive(Clone, Default)]
pub struct Controller(Arc<Mutex<Shared>>);
pub(crate) enum Observation {
    Url(String),
    Title(String),
    Loading(bool),
    Closed,
}
pub fn valid_url(value: &str) -> bool {
    value.len() <= 8192
        && !value.chars().any(char::is_control)
        && url::Url::parse(value).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
}
impl Controller {
    pub fn open(&self, key: &str, url: &str, title: &str) -> Result<(), ShellError> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || !valid_url(url)
            || title.len() > 4096
            || title.contains('\0')
        {
            return Err(ShellError("Invalid native browser target".into()));
        }
        let mut state = self.0.lock().unwrap();
        if state.closed || state.commands.len() >= 32 {
            return Err(ShellError("Native browser host closed or busy".into()));
        }
        if !state.entries.contains_key(key) && state.entries.len() >= LIMIT {
            return Err(ShellError("Close another reader tab first".into()));
        }
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| ShellError("Browser identities exhausted".into()))?;
        let generation = state.next;
        state.entries.insert(
            key.into(),
            (
                generation,
                Snapshot {
                    key: key.into(),
                    url: url.into(),
                    title: title.into(),
                    loading: true,
                    open: true,
                    error: None,
                },
            ),
        );
        state.commands.push_back(Command::Open {
            key: key.into(),
            url: url.into(),
            title: title.into(),
            generation,
        });
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
    pub fn action(&self, key: &str, action: Action) -> Result<(), ShellError> {
        let mut state = self.0.lock().unwrap();
        if state.closed || state.commands.len() >= 32 {
            return Err(ShellError("Native browser host closed or busy".into()));
        }
        if !state.entries.contains_key(key) {
            return Err(ShellError("The original browser tab is unavailable".into()));
        }
        state
            .commands
            .push_back(Command::Action(key.into(), action));
        if matches!(action, Action::Close) {
            state.entries.remove(key);
        }
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }
    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.0
            .lock()
            .unwrap()
            .entries
            .values()
            .map(|(_, v)| v.clone())
            .collect()
    }
    pub fn close_all(&self) {
        let mut state = self.0.lock().unwrap();
        state.commands.clear();
        state.observations.clear();
        let keys = state.entries.keys().cloned().collect::<Vec<_>>();
        state.entries.clear();
        for key in keys {
            state
                .commands
                .push_back(Command::Action(key, Action::Close));
        }
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn post(&self, key: &str, generation: u64, event: Observation) {
        let mut state = self.0.lock().unwrap();
        if state.closed
            || state
                .entries
                .get(key)
                .is_none_or(|(id, _)| *id != generation)
        {
            return;
        }
        state.observations.retain(|(original, _, old)| {
            original != key || std::mem::discriminant(old) != std::mem::discriminant(&event)
        });
        if state.observations.len() < 64 {
            state
                .observations
                .push_back((key.into(), generation, event));
        }
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    pub(super) fn observe(&self, key: &str, generation: u64, event: Observation) {
        let mut state = self.0.lock().unwrap();
        let Some((original, snapshot)) = state.entries.get_mut(key) else {
            return;
        };
        if *original != generation {
            return;
        }
        match event {
            Observation::Url(url) => snapshot.url = url,
            Observation::Title(title) => snapshot.title = title,
            Observation::Loading(loading) => snapshot.loading = loading,
            Observation::Closed => {
                snapshot.open = false;
                snapshot.loading = false;
            }
        }
    }
}
struct Page {
    generation: u64,
    webview: WebView,
    window: Window,
}
pub(super) struct Host {
    controller: Controller,
    pages: BTreeMap<String, Page>,
}
impl Host {
    pub fn new(controller: Controller, waker: Waker) -> Result<Self, ShellError> {
        {
            let mut state = controller.0.lock().unwrap();
            if state.waker.is_some() {
                return Err(ShellError(
                    "Native browser controller already attached".into(),
                ));
            }
            state.closed = false;
            state.waker = Some(waker);
        }
        Ok(Self {
            controller,
            pages: Default::default(),
        })
    }
    pub fn window_event(&mut self, id: WindowId, event: &WindowEvent<'_>) -> bool {
        let Some(key) = self
            .pages
            .iter()
            .find(|(_, p)| p.window.id() == id)
            .map(|(key, _)| key.clone())
        else {
            return false;
        };
        if matches!(event, WindowEvent::CloseRequested)
            && let Some(page) = self.pages.remove(&key)
        {
            self.controller
                .observe(&key, page.generation, Observation::Closed);
            drop(page);
        }
        true
    }
    pub fn poll(&mut self, target: &EventLoopWindowTarget<Message>) {
        let observations = {
            self.controller
                .0
                .lock()
                .unwrap()
                .observations
                .drain(..)
                .collect::<Vec<_>>()
        };
        for (key, generation, event) in observations {
            self.controller.observe(&key, generation, event);
        }
        let commands = {
            self.controller
                .0
                .lock()
                .unwrap()
                .commands
                .drain(..)
                .collect::<Vec<_>>()
        };
        for command in commands {
            match command {
                Command::Open {
                    key,
                    url,
                    title,
                    generation,
                } => {
                    if self.controller.0.lock().unwrap().entries.get(&key).is_none_or(|(current,_)|*current!=generation){continue;}
                    self.pages.remove(&key);
                    match page(target, &self.controller, &key, &url, &title, generation) {
                        Ok(page) => {
                            self.pages.insert(key, page);
                        }
                        Err(error) => {
                            let mut state = self.controller.0.lock().unwrap();
                            if let Some((id, snapshot)) = state.entries.get_mut(&key)
                                && *id == generation
                            {
                                snapshot.open = false;
                                snapshot.loading = false;
                                snapshot.error = Some(error.to_string());
                            }
                        }
                    }
                }
                Command::Action(key, action) => {
                    if matches!(action, Action::Close) {
                        self.pages.remove(&key);
                        continue;
                    }
                    let Some(page) = self.pages.get(&key) else {
                        continue;
                    };
                    let result = match action {
                        Action::Back => page.webview.evaluate_script("history.back()"),
                        Action::Forward => page.webview.evaluate_script("history.forward()"),
                        Action::Reload => page.webview.evaluate_script("location.reload()"),
                        Action::Show => {
                            page.window.set_visible(true);
                            page.window.set_focus();
                            Ok(())
                        }
                        Action::Hide => {
                            page.window.set_visible(false);
                            Ok(())
                        }
                        Action::Close => unreachable!(),
                    };
                    if let Err(error) = result {
                        let mut state = self.controller.0.lock().unwrap();
                        if let Some((_, snapshot)) = state.entries.get_mut(&key) {
                            snapshot.error = Some(error.to_string());
                        }
                    }
                }
            }
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let mut state = self.controller.0.lock().unwrap();
        state.closed = true;
        state.commands.clear();
        state.entries.clear();
        state.observations.clear();
        state.waker = None;
        drop(state);
        self.pages.clear();
    }
}
fn page(
    target: &EventLoopWindowTarget<Message>,
    controller: &Controller,
    key: &str,
    url: &str,
    title: &str,
    generation: u64,
) -> Result<Page, ShellError> {
    let window = WindowBuilder::new()
        .with_title(format!("{title} — Reader"))
        .with_inner_size(tao::dpi::LogicalSize::new(1000., 760.))
        .build(target)
        .map_err(|e| ShellError(e.to_string()))?;
    let nav_proxy = controller.clone();
    let nav_key = key.to_owned();
    let title_proxy = controller.clone();
    let title_key = key.to_owned();
    let load_proxy = controller.clone();
    let load_key = key.to_owned();
    let builder = WebViewBuilder::new()
        .with_incognito(true)
        .with_url(url)
        .with_navigation_handler(move |url| {
            if !valid_url(&url) {
                return false;
            }
            nav_proxy.post(&nav_key, generation, Observation::Url(url));
            true
        })
        .with_document_title_changed_handler(move |title| {
            let title = title.chars().filter(|value|!value.is_control()).take(512).collect();
            title_proxy.post(&title_key, generation, Observation::Title(title));
        })
        .with_on_page_load_handler(move |event, _| {
            load_proxy.post(
                &load_key,
                generation,
                Observation::Loading(matches!(event, wry::PageLoadEvent::Started)),
            );
        })
        .with_permission_handler(|_| wry::PermissionResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny);
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().expect("reader GTK container"))
    };
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window);
    Ok(Page {
        generation,
        webview: webview.map_err(|e| ShellError(e.to_string()))?,
        window,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_is_explicit_bounded_and_old_window_observations_cannot_rebind_a_tab() {
        let controller = Controller::default();
        for value in [
            "file:///private/a",
            "glory://localhost/index.html",
            "javascript:bad()",
            "https://user:secret@example.org",
        ] {
            assert!(controller.open("tab", value, "Reader").is_err());
        }
        controller
            .open("tab", "https://example.org/first", "First")
            .unwrap();
        let first = controller.0.lock().unwrap().entries["tab"].0;
        controller
            .open("tab", "https://example.org/second", "Second")
            .unwrap();
        controller.observe("tab", first, Observation::Title("stale".into()));
        assert_eq!(controller.snapshots()[0].title, "Second");
        for n in 1..LIMIT {
            controller
                .open(&format!("tab{n}"), "https://example.org/", "Reader")
                .unwrap();
        }
        assert!(
            controller
                .open("overflow", "https://example.org/", "Reader")
                .is_err()
        );
        controller.close_all();
        assert!(controller.snapshots().is_empty());
        assert_eq!(controller.0.lock().unwrap().commands.len(), LIMIT);
    }
}
