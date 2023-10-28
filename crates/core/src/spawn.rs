use std::{future::Future, sync::OnceLock};

cfg_feature! {
    #![all(feature = "web-ssr", not(feature = "__single_holder"))]

    use std::cell::RefCell;
    use std::pin::Pin; use std::cell::OnceCell;

    use once_cell::sync::Lazy;use parking_lot::RwLock;

    use indexmap::IndexMap;
    use futures::StreamExt;
    use futures_channel::mpsc::{self, UnboundedSender};
    use tokio_util::task::LocalPoolHandle;

    use crate::HolderId;

    pub(crate) static NOTIFIER: OnceLock<UnboundedSender<Pin<Box<dyn Future<Output = ()> + 'static>>>> = OnceLock::new();

    fn get_task_pool() -> LocalPoolHandle {
        static LOCAL_POOL: OnceLock<LocalPoolHandle> = OnceLock::new();
        LOCAL_POOL
            .get_or_init(|| {
                tokio_util::task::LocalPoolHandle::new(
                    std::thread::available_parallelism().map(Into::into).unwrap_or(1),
                )
            })
            .clone()
    }
    pub async fn start_serice() {
        let (tx, mut rx) = mpsc::unbounded::<()>();
        NOTIFIER.set(tx);
        tokio::spawn(async move {
            while let Some(fut) = rx.next().await {
              fut.await;
                // for task in tasks {
                //     let pool_handle = get_task_pool();
                //     pool_handle.spawn_pinned(move || task );
                // }
                // let local = tokio::task::LocalSet::new();
                // for task in tasks {
                //     local.spawn_local(async move {
                //         task.await;
                //     });
                // }
                // local.await;
            }
        });
    }

    fn notify(fut: Pin<Box<dyn Future<Output = ()> + 'static>>) {
        NOTIFIER.get().unwrap().unbounded_send(fut).unwrap();
    }
}

/// Spawns and runs a thread-local [`Future`] in a platform-independent way.
///
/// This can be used to interface with any `async` code by spawning a task
/// to run a `Future`.
pub fn spawn_local<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            wasm_bindgen_futures::spawn_local(fut)
        } else if #[cfg(any(test, doctest))] {
            tokio_test::block_on(fut);
        } else if #[cfg(all(feature = "web-ssr", not(feature = "__single_holder")))] {
            // LOCAL_TASKS.with_borrow_mut(|tasks| {
            //     tasks.push(Box::pin(fut));
            // });
            notify(Box::pin(fut));
        }  else {
            futures::executor::block_on(fut)
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
