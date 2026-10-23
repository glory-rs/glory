//! The client runs in a separate process; the event loop keeps servicing UIA.
use super::*;
use glory_ui::when;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};

#[test]
#[ignore = "requires an interactive Windows desktop and .NET UI Automation"]
fn real_accesskit_windows_ui_automation() {
    let mut builder = EventLoop::builder();
    winit::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    let event_loop = builder.build().unwrap();
    let (proxy, queue) = BlitzShellProxy::new(event_loop.create_proxy());
    let mut shell = Shell {
        accessibility: Default::default(),
        inner: BlitzApplication::new(proxy.clone(), queue),
        event_loop: None,
        error: Rc::default(),
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();
    let titles = [
        format!("Glory AccessKit A {nonce}"),
        format!("Glory AccessKit B {nonce}"),
    ];
    let mut counters = Vec::new();
    let mut drafts = Vec::new();
    let mut checks = Vec::new();
    let mut renderers = Vec::new();
    let mut reads = Vec::new();
    for title in &titles {
        let app = WindowApp::new();
        let count = app.app.scope().cage(0u32);
        let draft = app.app.scope().cage(String::from("draft🙂"));
        let checked = app.app.scope().cage(false);
        let position = app.app.scope().cage(0.0);
        let seek = app.app.scope().cage(false);
        let expanded = app.app.scope().cage(false);
        let activations = app.app.scope().cage(0u32);
        let target = app.app.scope().cage(None);
        let present = app.app.scope().cage(true);
        let disclosure = app.app.scope().bond(move || {
            target.get().map(|controls| glory_render::Disclosure {
                expanded: expanded.get(),
                controls,
            })
        });
        let source_reads = Rc::new(Cell::new(0));
        let observed = source_reads.clone();
        app.mount(
            view().name("Controls").fill((
                group().fill((
                    button()
                        .label("Disclosure control")
                        .disclosure(disclosure)
                        .on(EventKind::Activate, move |_| {
                            expanded.revise(|v| *v = !*v);
                            activations.revise(|n| *n += 1);
                        }),
                    text().content(activations.map(|n| format!("Disclosure activations {n}"))),
                    when(present, move || {
                        view()
                            .name("Disclosure target")
                            .node_ref(target)
                            .fill(text().content("Drawer contents"))
                    }),
                    button()
                        .label("Retire disclosure target")
                        .on(EventKind::Activate, move |_| present.revise(|v| *v = false)),
                )),
                button()
                    .label("Increment")
                    .on(EventKind::Activate, move |_| count.revise(|n| *n += 1)),
                text().content(count.map(|n| format!("Count {n}"))),
                input()
                    .name("Editor")
                    .value(draft)
                    .on(EventKind::ValueChanged, move |e| {
                        if let Event::ValueChanged { value, .. } = e {
                            draft.revise(|v| *v = value);
                        }
                    }),
                check()
                    .label("Choice")
                    .checked(checked)
                    .on(EventKind::CheckedChanged, move |e| {
                        if let Event::CheckedChanged(value) = e {
                            checked.revise(|v| *v = value);
                        }
                    }),
                input()
                    .name("Secret")
                    .purpose(Purpose::Password)
                    .value("never-export🙂"),
                button()
                    .label("Disabled")
                    .enabled(false)
                    .on(EventKind::Activate, move |_| count.revise(|n| *n += 100)),
                list(
                    100_000_000usize,
                    move |index| {
                        observed.set(observed.get() + 1);
                        index
                    },
                    move |_, key| {
                        input()
                            .name(format!("Row editor {key}"))
                            .value(format!("draft {key}"))
                            .on(EventKind::FocusChanged, move |event| {
                                if key == 0
                                    && matches!(event, Event::FocusChanged(true))
                                    && seek.get()
                                {
                                    seek.revise(|v| *v = false);
                                    position.revise(|p| *p = 90_000_000.0 * 24.0);
                                }
                            })
                    },
                )
                .name("Rows")
                .height(120.0)
                .row_height(24.0)
                .overscan(2)
                .position(position)
                .locate(|key| Some(*key))
                .inspect(|stats| {
                    eprintln!(
                        "UIA list: range={:?}, rows={}, pinned={}",
                        stats.range, stats.rows, stats.pinned
                    )
                }),
                button()
                    .label("Arm far scroll")
                    .on(EventKind::Activate, move |_| seek.revise(|v| *v = true)),
            )),
        )
        .unwrap();
        counters.push(count);
        drafts.push(draft);
        checks.push(checked);
        renderers.push(app.renderer.clone());
        reads.push(source_reads);
        shell.add_window(
            app,
            WindowAttributes::default()
                .with_title(title)
                .with_surface_size(winit::dpi::PhysicalSize::new(480, 660)),
        );
    }
    let report = std::path::PathBuf::from(format!(
        "D:/BuildCache/glory-semantic-ui/accesskit-uia-{nonce}.json"
    ));
    eprintln!("UIA report: {}", report.display());
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/check-semantic-accesskit-uia.ps1")
        .canonicalize()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let helper_proxy = proxy.clone();
    let pid = std::process::id();
    let helper = std::thread::spawn(move || {
        use std::os::windows::process::CommandExt;
        let diagnostics = std::fs::File::create(report.with_extension("client.log")).unwrap();
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-TestProcessId")
            .arg(pid.to_string())
            .arg("-FirstTitle")
            .arg(&titles[0])
            .arg("-SecondTitle")
            .arg(&titles[1])
            .arg("-Report")
            .arg(report)
            .stdin(Stdio::null())
            .stdout(diagnostics.try_clone().unwrap())
            .stderr(diagnostics)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(35);
        let result = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status.success();
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            std::thread::sleep(Duration::from_millis(30));
        };
        tx.send(result).unwrap();
        helper_proxy.wake_up();
    });
    struct Harness {
        shell: Shell,
        rx: std::sync::mpsc::Receiver<bool>,
        done: Rc<Cell<Option<bool>>>,
    }
    impl ApplicationHandler for Harness {
        fn can_create_surfaces(&mut self, e: &dyn ActiveEventLoop) {
            self.shell.can_create_surfaces(e);
        }
        fn destroy_surfaces(&mut self, e: &dyn ActiveEventLoop) {
            self.shell.destroy_surfaces(e);
        }
        fn window_event(&mut self, e: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
            self.shell.window_event(e, id, event);
        }
        fn proxy_wake_up(&mut self, e: &dyn ActiveEventLoop) {
            self.shell.proxy_wake_up(e);
            if let Ok(result) = self.rx.try_recv() {
                self.done.set(Some(result));
                for id in self.shell.inner.windows.keys().copied().collect::<Vec<_>>() {
                    self.shell
                        .inner
                        .proxy
                        .send_event(BlitzShellEvent::CloseWindow { window_id: id });
                }
            }
        }
    }
    let done = Rc::new(Cell::new(None));
    event_loop
        .run_app(Harness {
            shell,
            rx,
            done: done.clone(),
        })
        .unwrap();
    helper.join().unwrap();
    assert_eq!(
        done.get(),
        Some(true),
        "UI Automation client failed; see the retained report"
    );
    assert!(counters.iter().all(|c| c.try_get().is_err()));
    assert!(drafts.iter().all(|c| c.try_get().is_err()));
    assert!(checks.iter().all(|c| c.try_get().is_err()));
    assert!(renderers.iter().all(|r| r.stats().nodes == 0));
    assert!(reads.iter().all(|r| r.get() < 100));
    eprintln!(
        "Native UIA: two isolated windows, Invoke/Toggle/SetValue/Focus/password/list bounds, distant row reveal, Disclosure Expanded/ControllerFor/actions/retirement and owner cleanup passed"
    );
}
