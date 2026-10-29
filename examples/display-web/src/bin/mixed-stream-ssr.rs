#[cfg(not(target_arch = "wasm32"))]
#[path = "../stream_mixed.rs"]
mod fixture;
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use futures_core::Stream;
    use std::{
        cell::RefCell,
        io::{self, BufRead, Write},
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };
    #[derive(Default)]
    struct Gate {
        stage: usize,
        waker: Option<Waker>,
    }
    let gate = Rc::new(RefCell::new(Gate::default()));
    let pending = gate.clone();
    let mut model = None;
    let mut stream = g::ssr::stream_with(
        g::resources::Resources::default(),
        g::ssr::stream::Config::default(),
        |scope, _| {
            let value = Rc::new(fixture::Model::new(scope, 0));
            let view = fixture::view(&value);
            model = Some(value.clone());
            scope
                .spawn(async move {
                    for stage in 1..=3 {
                        let gate = pending.clone();
                        std::future::poll_fn(move |context| {
                            let mut gate = gate.borrow_mut();
                            if gate.stage >= stage {
                                Poll::Ready(())
                            } else {
                                gate.waker = Some(context.waker().clone());
                                Poll::Pending
                            }
                        })
                        .await;
                        value.advance(stage);
                    }
                })
                .unwrap();
            view
        },
    )
    .unwrap();
    let bootstrap = g::ssr::stream::Bootstrap {
        root_id: "stream-root".into(),
        data_id: "stream-data".into(),
        script_url: "/stream.js".into(),
        nonce: None,
    };
    let mut output = io::stdout().lock();
    write!(
        output,
        "{}{}<script type=\"module\" async src=\"/loader.js\"></script>",
        g::ssr::Document::default().opening_html().unwrap(),
        bootstrap.head_script().unwrap()
    )
    .unwrap();
    let mut input = io::stdin().lock();
    let mut completed = false;
    loop {
        match Pin::new(&mut stream).poll_next(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(Some(Ok(frame))) => {
                write!(output, "{}", frame.to_html(&bootstrap).unwrap()).unwrap();
                output.flush().unwrap();
                if frame.kind() == g::ssr::stream::FrameKind::End {
                    completed = true;
                    break;
                }
            }
            Poll::Ready(Some(Err(error))) => panic!("{error}"),
            Poll::Ready(None) => break,
            Poll::Pending => {
                let mut command = String::new();
                if input.read_line(&mut command).unwrap() == 0 {
                    break;
                }
                let stage: usize = command.trim().parse().unwrap();
                let waker = {
                    let mut gate = gate.borrow_mut();
                    gate.stage = stage;
                    gate.waker.take()
                };
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
        }
    }
    stream.cancel();
    write!(output, "</body></html>").unwrap();
    output.flush().unwrap();
    let state = model.unwrap();
    eprintln!(
        "{{\"completed\":{completed},\"nodes\":{},\"tasks\":{},\"drops\":{}}}",
        stream.stats().nodes,
        stream.stats().tasks,
        state.drops.get()
    );
}
#[cfg(target_arch = "wasm32")]
fn main() {}
