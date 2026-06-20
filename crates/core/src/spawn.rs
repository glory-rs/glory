use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;
#[cfg(not(target_arch = "wasm32"))]
use std::pin::Pin;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    /// When `Some`, [`spawn_local`] *defers* server-side futures into this
    /// queue instead of blocking on them. Streaming SSR
    /// ([`crate::web::holders::ServerHolder::new_streaming`]) arms this so the
    /// initial shell — with Suspense fallbacks still pending — can flush
    /// before any async resource resolves, then drains the queue to stream
    /// resolved patches. Untouched (`None`) for every non-streaming path, so
    /// default SSR keeps its blocking-resolve semantics.
    static DEFERRED: RefCell<Option<Vec<Pin<Box<dyn Future<Output = ()>>>>>> = const { RefCell::new(None) };
}

/// Spawns and runs a thread-local [`Future`] in a platform-independent way.
///
/// This can be used to interface with any `async` code by spawning a task
/// to run a `Future`.
pub fn spawn_local<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Streaming SSR defers async work so the shell can flush first.
        let armed = DEFERRED.with(|slot| slot.borrow().is_some());
        if armed {
            DEFERRED.with(|slot| {
                if let Some(queue) = slot.borrow_mut().as_mut() {
                    queue.push(Box::pin(fut) as Pin<Box<dyn Future<Output = ()>>>);
                }
            });
            return;
        }
        return spawn_local_blocking(fut);
    }
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut)
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_local_blocking<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    cfg_if! {
        if #[cfg(all(feature = "web-ssr", not(feature = "single-app"), not(test)))] {
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    let local = tokio::task::LocalSet::new();
                    local.spawn_local(fut);
                    local.await;
                });
            });
        }  else {
            futures::executor::block_on(fut)
        }
    }
}

// The deferral driver below is only reachable from server-side streaming SSR
// (`ServerHolder::new_streaming`), so it is gated to that build to stay
// dead-code-free elsewhere.
cfg_feature! {
    #![all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32"))]

    /// Arms the streaming deferral queue, replacing any existing queue with an
    /// empty one and returning the previous queue contents (if armed).
    ///
    /// Returns the previously-armed state so the caller can restore it, keeping
    /// nested streaming renders well-behaved.
    pub(crate) fn begin_deferred() -> Option<Vec<Pin<Box<dyn Future<Output = ()>>>>> {
        DEFERRED.with(|slot| slot.borrow_mut().replace(Vec::new()))
    }

    /// Takes the futures deferred so far, leaving the queue armed-but-empty so
    /// futures spawned while draining (suspense waterfalls) keep deferring.
    fn take_deferred() -> Vec<Pin<Box<dyn Future<Output = ()>>>> {
        DEFERRED.with(|slot| {
            let mut slot = slot.borrow_mut();
            match slot.as_mut() {
                Some(queue) => std::mem::take(queue),
                None => Vec::new(),
            }
        })
    }

    /// Disarms the deferral queue, restoring `previous` (typically the value
    /// returned by [`begin_deferred`]).
    pub(crate) fn end_deferred(previous: Option<Vec<Pin<Box<dyn Future<Output = ()>>>>>) {
        DEFERRED.with(|slot| *slot.borrow_mut() = previous);
    }

    /// Drives every deferred future to completion, re-collecting any futures
    /// that later deferral (suspense waterfalls) produces, until the queue
    /// stays empty.
    ///
    /// The deferral queue must already be armed (see [`begin_deferred`]); it is
    /// left armed-but-empty on return so the caller can [`end_deferred`].
    pub(crate) fn drive_deferred() {
        loop {
            let batch = take_deferred();
            if batch.is_empty() {
                break;
            }
            run_deferred_batch(batch);
        }
    }

    fn run_deferred_batch(batch: Vec<Pin<Box<dyn Future<Output = ()>>>>) {
        cfg_if! {
            if #[cfg(not(test))] {
                tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async move {
                        let local = tokio::task::LocalSet::new();
                        for fut in batch {
                            local.spawn_local(fut);
                        }
                        local.await;
                    });
                });
            } else {
                let mut pool = futures::executor::LocalPool::new();
                {
                    use futures::task::LocalSpawnExt;
                    let spawner = pool.spawner();
                    for fut in batch {
                        spawner.spawn_local(fut).expect("local pool accepts spawned future");
                    }
                }
                pool.run();
            }
        }
    }
}

/// The microtask is a short function which will run after the current task has
/// completed its work and when there is no other code waiting to be run before
/// control of the execution context is returned to the browser's event loop.
///
/// Microtasks are especially useful for libraries and frameworks that need
/// to perform final cleanup or other just-before-rendering tasks.
///
/// [MDN queueMicrotask](https://developer.mozilla.org/en-US/docs/Web/API/queueMicrotask)
pub fn queue_microtask(task: impl FnOnce() + 'static) {
    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
    {
        task();
    }

    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    {
        use js_sys::{Function, Reflect};
        use wasm_bindgen::prelude::*;

        let task = Closure::once_into_js(task);
        let window = web_sys::window().expect("window not available");
        let queue_microtask = Reflect::get(&window, &JsValue::from_str("queueMicrotask")).expect("queueMicrotask not available");
        let queue_microtask = queue_microtask.unchecked_into::<Function>();
        _ = queue_microtask.call1(&JsValue::UNDEFINED, &task);
    }
}
