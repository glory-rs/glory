//! A separate main-thread process: libtest workers cannot own AppKit's loop.
#[cfg(target_os = "macos")]
fn main() {
    use glory_apace::shell::{Shell, WindowApp, WindowAttributes};
    use glory_ui::{Widget, text, widget};
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };
    use wingy::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    };
    if std::env::var_os("GLORY_MACOS_WINDOW").is_none() {
        println!("macOS window acceptance skipped; set GLORY_MACOS_WINDOW=1 explicitly");
        return;
    }
    struct Driver {
        shell: Shell,
        started: Instant,
        phase: u8,
        live: Rc<Cell<usize>>,
        done: Rc<Cell<bool>>,
    }
    impl ApplicationHandler for Driver {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            self.shell.can_create_surfaces(event_loop);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
        }
        fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
            self.shell.proxy_wake_up(event_loop);
        }
        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            id: WindowId,
            event: WindowEvent,
        ) {
            self.shell.window_event(event_loop, id, event);
            if self.started.elapsed() > Duration::from_secs(20) {
                panic!("native presentation timed out");
            }
            let Some(stats) = self.shell.window_stats(id) else {
                return;
            };
            if stats.presented_frames == 0 {
                return;
            }
            assert!(stats.surface_size.width > 0 && stats.surface_size.height > 0);
            assert!(stats.scale_factor.is_finite() && stats.scale_factor > 0.0);
            if self.phase == 0 {
                self.phase = 1;
                self.shell
                    .request_size(id, wingy::dpi::PhysicalSize::new(640, 480).into())
                    .unwrap();
            } else if stats.surface_size == wingy::dpi::PhysicalSize::new(640, 480)
                && stats.presented_frames >= 2
            {
                println!("macOS native present/resize: {stats:?}");
                self.shell
                    .window_event(event_loop, id, WindowEvent::CloseRequested);
                assert_eq!(self.live.get(), 0, "window Owner cleanup");
                self.done.set(true);
                event_loop.exit();
            }
        }
        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            assert!(
                self.started.elapsed() < Duration::from_secs(20),
                "native presentation timed out"
            );
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
        }
    }
    let event_loop = EventLoop::builder().build().unwrap();
    let mut shell = Shell::from_event_loop(event_loop);
    let event_loop = shell.take_event_loop().unwrap();
    let live = Rc::new(Cell::new(0));
    let mounted = live.clone();
    let app = WindowApp::new();
    app.mount(widget(move |scope| {
        mounted.set(mounted.get() + 1);
        let cleanup = mounted.clone();
        scope.on_cleanup(move || cleanup.set(cleanup.get() - 1))?;
        text()
            .content("Glory macOS native acceptance")
            .show_in(scope)
    }))
    .unwrap();
    shell.add_window(
        app,
        WindowAttributes::default().with_title("Glory macOS acceptance"),
    );
    let done = Rc::new(Cell::new(false));
    event_loop
        .run_app(Driver {
            shell,
            started: Instant::now(),
            phase: 0,
            live: live.clone(),
            done: done.clone(),
        })
        .unwrap();
    assert!(done.get());
    assert_eq!(live.get(), 0);
}
#[cfg(not(target_os = "macos"))]
fn main() {}
