//! Real browser keyboard routing through a dialog ancestor and controlled editors.
use glory::prelude::*;
use glory::render::{Event, EventKind, KeyboardCommand};
use glory::web::WebApp;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
thread_local! {static APP: RefCell<Option<WebApp>> = const { RefCell::new(None) };}
pub(crate) fn clear() {
    drop(APP.with(|app| app.borrow_mut().take()));
}
#[wasm_bindgen]
pub fn keyboard_mount() -> std::result::Result<(), JsValue> {
    crate::browser::unmount();
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("app")
        .unwrap();
    let web = glory::web::launch_with(root, |scope| {
        let dismissed = scope.cage(0usize);
        let submitted = scope.cage(0usize);
        view()
            .on(EventKind::Keyboard, move |event| {
                if let Event::Keyboard {
                    command: KeyboardCommand::Dismiss,
                    ..
                } = event
                {
                    dismissed.revise(|n| *n += 1);
                }
            })
            .fill((
                input().name("Dialog draft").multiline(true),
                input().name("Explicit submit").multiline(true).on(
                    EventKind::Keyboard,
                    move |event| {
                        if let Event::Keyboard {
                            command: KeyboardCommand::Submit,
                            ..
                        } = event
                        {
                            submitted.revise(|n| *n += 1);
                        }
                    },
                ),
                text()
                    .class("dismiss-count")
                    .content(dismissed.map(|n| n.to_string())),
                text()
                    .class("submit-count")
                    .content(submitted.map(|n| n.to_string())),
            ))
    })
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    APP.with(|app| *app.borrow_mut() = Some(web));
    Ok(())
}
