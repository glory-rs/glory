//! Real system WebView acceptance, opt in on the platform's UI/main thread.
use glory_render::{Query, QueryValue, TextSelection, style::*};
use glory_ui::{events::*, *};
use glory_wry::shell::{Config, Shell, ShellError, WindowApp};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

// Fixed CSS sizes can be quantized by the WebView's 1/64 layout units at
// fractional effective scale. The measured WebView2 DPR=1.02 returns 359.9877319
// for both this fixture and an independent plain DOM width:360px probe.
// Keep this bound below a CSS pixel; do not apply it to selection/scroll data.
fn fixed_layout_dimension(actual: f64, expected: f64) -> bool {
    actual.is_finite() && actual > 0. && (actual - expected).abs() <= 1. / 64.
}

pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    assert!(fixed_layout_dimension(359.98773193359375, 360.));
    for invalid in [359., 361., 0., f64::NAN, f64::INFINITY] {
        assert!(!fixed_layout_dimension(invalid, 360.));
    }
    if std::env::var("GLORY_WRY_WINDOW").as_deref() != Ok("1") {
        println!("Set GLORY_WRY_WINDOW=1 to run actual system WebView acceptance.");
        return Ok(());
    }
    let app = WindowApp::new();
    let scope = app.app().scope();
    let count = scope.cage(0);
    let value = scope.cage("initial".to_owned());
    let selected = scope.cage(false);
    let captured = scope.cage(String::new());
    let shown = scope.cage(true);
    let input_node = scope.cage(None);
    let box_node = scope.cage(None);
    let done = Rc::new(Cell::new(false));
    let complete = done.clone();
    let committed = Rc::new(RefCell::new(String::new()));
    let commit_value = committed.clone();
    let query_count = Rc::new(Cell::new(0));
    let requested = Cell::new(false);
    let async_value = scope.cage(String::from("waiting"));
    let weak = scope.downgrade();
    let moving = Rc::new(
        group()
            .fill(input().class("moving").name("Moving").value("stable"))
            .mount(scope)?,
    );
    let move_range = moving.clone();
    let park = moving.clone();
    let resume = moving.clone();
    let cancelled = Rc::new(Cell::new(false));
    struct Guard(Rc<Cell<bool>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let guard = Guard(cancelled.clone());
    scope.spawn(async move {
        std::future::pending::<()>().await;
        drop(guard);
    })?;
    let output = app.renderer().clone();
    let author = output.clone();
    app.mount(
        button()
            .class("theme")
            .label("theme")
            .on(activate, move |_| {
                author.set_author_css(".echo {color:rgb(1,2,3)}").unwrap();
            }),
    )?;
    app.mount(
        col()
            .class("test-root")
            .node_ref(box_node)
            .style(
                Style::new()
                    .width(Length::Px(360.))
                    .height(Length::Px(260.)),
            )
            .fill((
                text()
                    .class("count")
                    .content(count.map(|n| format!("Count:{n}"))),
                input()
                    .node_ref(input_node)
                    .name("Message")
                    .value(value)
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|v| *v = next);
                        }
                    })
                    .on(commit, move |event| {
                        if let Event::Commit { value, .. } = event {
                            *commit_value.borrow_mut() = value;
                        }
                    }),
                text().class("echo").content(value),
                text().class("captured").content(captured),
                button()
                    .class("capture")
                    .label("capture")
                    .on(activate, move |_| captured.revise(|v| *v = value.get())),
                check()
                    .label("Check")
                    .checked(selected)
                    .on(checked_changed, move |event| {
                        if let Event::CheckedChanged(next) = event {
                            selected.revise(|v| *v = next);
                        }
                    }),
                button()
                    .class("increment")
                    .label("+1")
                    .on(activate, move |_| count.revise(|n| *n += 1)),
                button()
                    .class("toggle")
                    .label("toggle")
                    .on(activate, move |_| shown.revise(|v| *v = !*v)),
                when(shown, || text().class("conditional").content("temporary")),
                button()
                    .class("done")
                    .label("done")
                    .on(activate, move |_| complete.set(true)),
            )),
    )?;
    app.mount(
        group().fill((
            text().class("async-value").content(async_value),
            button()
                .class("async-start")
                .label("async")
                .on(activate, move |_| {
                    use std::sync::{
                        Arc,
                        atomic::{AtomicBool, Ordering},
                    };
                    use std::task::Poll;
                    let ready = Arc::new(AtomicBool::new(false));
                    let mut started = false;
                    weak.upgrade()
                        .unwrap()
                        .spawn(async move {
                            std::future::poll_fn(move |context| {
                                if ready.load(Ordering::Acquire) {
                                    return Poll::Ready(());
                                }
                                if !started {
                                    started = true;
                                    let ready = ready.clone();
                                    let waker = context.waker().clone();
                                    std::thread::spawn(move || {
                                        std::thread::sleep(Duration::from_millis(80));
                                        ready.store(true, Ordering::Release);
                                        waker.wake();
                                    });
                                }
                                Poll::Pending
                            })
                            .await;
                            async_value.revise(|value| *value = "awake".into());
                        })
                        .unwrap();
                }),
        )),
    )?;
    app.mount(
        row().fill((
            button()
                .class("move")
                .label("move")
                .on(activate, move |_| move_range.place_before(None).unwrap()),
            button()
                .class("park")
                .label("park")
                .on(activate, move |_| park.suspend().unwrap()),
            button()
                .class("resume")
                .label("resume")
                .on(activate, move |_| resume.place_before(None).unwrap()),
        )),
    )?;
    let start = Instant::now();
    Shell::new().run_until(
        app,
        Config {
            title: "Glory Wry acceptance".into(),
            initialization_script: Some(include_str!("../window.js").into()),
            ..Default::default()
        },
        |window| {
            if start.elapsed() > Duration::from_secs(20) {
                return Err(ShellError("native WebView acceptance timeout".into()));
            }
            if done.get() && !requested.replace(true) {
                if captured.get() != "rapid native input 0123456789" {
                    return Err(ShellError("activation overtook pending native text".into()));
                }
                if count.get() != 1
                    || value.get() != "中文😀"
                    || !selected.get()
                    || committed.borrow().as_str() != "中文😀"
                {
                    return Err(ShellError(format!(
                        "native Rust observations disagree: {}, {:?}, {}, {:?}",
                        count.get(),
                        value.get(),
                        selected.get(),
                        committed.borrow()
                    )));
                }
                let input = input_node.get().expect("input mounted");
                let container = box_node.get().expect("box mounted");
                window.app().scope().set_input_selection(
                    input,
                    TextSelection {
                        start: 2,
                        end: 4,
                        ..Default::default()
                    },
                )?;
                window.app().flush()?;
                for query in [
                    Query::Focus(input),
                    Query::Selection(input),
                    Query::Bounds(container),
                    Query::ScrollMetrics(container),
                ] {
                    let replies = query_count.clone();
                    window.app().request(query, move |reply| {
                        match reply.expect("real WebView query") {
                            QueryValue::Focus(focused) => assert!(focused),
                            QueryValue::Selection { start, end } => {
                                assert_eq!((start, end), (2, 4))
                            }
                            QueryValue::Bounds(rect) => {
                                assert!(fixed_layout_dimension(rect.width, 360.), "{rect:?}");
                                assert!(fixed_layout_dimension(rect.height, 260.), "{rect:?}");
                            }
                            QueryValue::ScrollMetrics(metrics) => {
                                assert_eq!(metrics.viewport_width, 360.)
                            }
                            other => panic!("unexpected query {other:?}"),
                        }
                        replies.set(replies.get() + 1);
                    })?;
                }
            }
            Ok(query_count.get() == 4 && !window.renderer().in_flight())
        },
    )?;
    assert_eq!(query_count.get(), 4);
    assert!(cancelled.get(), "closing must drop pending owned futures");
    assert!(
        output.take_script().is_err(),
        "closing must revoke retained renderer handles"
    );
    println!(
        "GLORY_WRY_WINDOW_OK: native state, IPC, controlled input/UTF-16, commit, checks, conditional disposal, focus and geometry"
    );
    Ok(())
}
