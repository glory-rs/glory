use crate::{Location, Navigation, NavigationKind, Result, RouterHandle};
use std::{cell::RefCell, rc::Rc};

/// Compose the existing portable Link. Its real href works before WASM; the
/// semantic request marker suppresses only an ordinary activation's default.
/// The application explicitly receives blocked/failed navigation results.
/// This synchronous helper reports AsyncRequired for asynchronous guards; use
/// link_with to select the mounted Owner's explicit task execution service.
pub fn link(
    router: RouterHandle,
    location: Location,
    mut on_result: impl FnMut(Result<Navigation>) + 'static,
) -> glory_ui::Link {
    glory_ui::link()
        .target(location.to_string())
        .semantics(Some(Box::new(glory_render::Semantics {
            link_request: true,
            ..Default::default()
        })))
        .on(glory_ui::events::activate, move |_| {
            on_result(router.push(location.clone()))
        })
}

/// Compose a portable Link with an explicit Owner and its installed task Host.
/// Synchronous guards finish during activation; async guards are queued without
/// polling or performing I/O in the native event listener. Retiring the Owner
/// or releasing the Link handler cancels queued/pending navigation and replies.
pub fn link_with(
    owner: &glory_core::Scope,
    router: RouterHandle,
    location: Location,
    on_result: impl FnMut(Result<Navigation>) + 'static,
) -> Result<glory_ui::Link> {
    owner.owner().check_write()?;
    if router.runtime()? != owner.runtime() {
        return Err(crate::Error::WrongRuntime);
    }
    let tasks = Rc::new(owner.child());
    let lifetime = tasks.id();
    let reply = Rc::new(RefCell::new(on_result));
    Ok(glory_ui::link()
        .target(location.to_string())
        .semantics(Some(Box::new(glory_render::Semantics {
            link_request: true,
            ..Default::default()
        })))
        .on(glory_ui::events::activate, move |_| {
            if !lifetime.is_alive() {
                return;
            }
            let asynchronous = match router.has_async_guards() {
                Ok(asynchronous) => asynchronous,
                Err(error) => {
                    (reply.borrow_mut())(Err(error));
                    return;
                }
            };
            if !asynchronous {
                let prepared = router.prepare(location.clone(), NavigationKind::Push);
                if lifetime.is_alive() {
                    let result = prepared.and_then(|prepared| prepared.commit());
                    if lifetime.is_alive() {
                        (reply.borrow_mut())(result);
                    }
                }
                return;
            }
            let router = router.clone();
            let location = location.clone();
            let callback = reply.clone();
            if let Err(error) = tasks.spawn(async move {
                let prepared = router.prepare_async(location, NavigationKind::Push).await;
                // A guard/provider can synchronously retire the initiating
                // Owner while this task is being polled. Task cancellation
                // cannot drop its active stack, so revalidate before commit.
                if lifetime.is_alive() {
                    let result = prepared.and_then(|prepared| prepared.commit());
                    if lifetime.is_alive() {
                        (callback.borrow_mut())(result);
                    }
                }
            }) && lifetime.is_alive()
            {
                (reply.borrow_mut())(Err(error.into()));
            }
        }))
}
