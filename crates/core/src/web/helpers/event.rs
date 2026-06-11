use std::{borrow::Cow, cell::RefCell, collections::HashSet};
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt, closure::Closure, convert::FromWasmAbi, intern};

thread_local! {
    pub static GLOBAL_EVENTS: RefCell<HashSet<Cow<'static, str>>> = RefCell::new(HashSet::new());
}

/// Adds an event listener to the target DOM element using implicit event delegation.
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn add_event_listener<E>(
    target: &web_sys::Element,
    event_name: Cow<'static, str>,
    #[cfg(debug_assertions)] mut cb: impl FnMut(E) + 'static,
    #[cfg(not(debug_assertions))] cb: impl FnMut(E) + 'static,
) where
    E: FromWasmAbi + 'static,
{
    cfg_if! {
      if #[cfg(debug_assertions)] {
        let span = ::tracing::Span::current();
        let cb = move |e| {
          let _guard = span.enter();
          cb(e);
        };
      }
    }

    let cb = Closure::wrap(Box::new(cb) as Box<dyn FnMut(E)>).into_js_value();
    let key = event_delegation_key(&event_name);
    _ = js_sys::Reflect::set(target, &JsValue::from_str(&key), &cb);
    add_delegated_event_listener(event_name);
}

#[doc(hidden)]
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn add_event_listener_undelegated<E>(target: &web_sys::Element, event_name: &str, cb: impl FnMut(E) + 'static)
where
    E: FromWasmAbi + 'static,
{
    let event_name = intern(event_name);
    let cb = Closure::wrap(Box::new(cb) as Box<dyn FnMut(E)>).into_js_value();
    _ = target.add_event_listener_with_callback(event_name, cb.unchecked_ref());
}

// cf eventHandler in ryansolid/dom-expressions
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub(crate) fn add_delegated_event_listener(event_name: Cow<'static, str>) {
    GLOBAL_EVENTS.with_borrow_mut(|global_events| {
        if !global_events.contains(&event_name) {
            // create global handler
            let key = JsValue::from_str(&event_delegation_key(&event_name));
            let handler = move |ev: web_sys::Event| {
                let path = ev.composed_path();
                let path_len = path.length();

                for index in 0..path_len {
                    let node = path.get(index);
                    if node.is_undefined() || node.is_null() {
                        continue;
                    }

                    if node.dyn_ref::<web_sys::Element>().is_none() {
                        continue;
                    }

                    let node_is_disabled = js_sys::Reflect::get(&node, &JsValue::from_str("disabled")).unwrap_throw().is_truthy();
                    if node_is_disabled {
                        continue;
                    }

                    let maybe_handler = js_sys::Reflect::get(&node, &key).unwrap_throw();
                    if !maybe_handler.is_undefined() {
                        let f = maybe_handler.unchecked_ref::<js_sys::Function>();
                        with_current_target(&ev, &node, || {
                            let _ = f.call1(&node, &ev);
                        });

                        if ev.cancel_bubble() {
                            return;
                        }
                    }
                }
            };

            cfg_if! {
              if #[cfg(debug_assertions)] {
                let span = ::tracing::Span::current();
                let handler = move |e| {
                  let _guard = span.enter();
                  handler(e);
                };
              }
            }

            let handler = Box::new(handler) as Box<dyn FnMut(web_sys::Event)>;
            let handler = Closure::wrap(handler).into_js_value();
            _ = crate::web::window().add_event_listener_with_callback(&event_name, handler.unchecked_ref());

            // register that we've created handler
            global_events.insert(event_name);
        }
    })
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub(crate) fn event_delegation_key(event_name: &str) -> String {
    let event_name = intern(event_name);
    let mut n = String::from("$$$");
    n.push_str(event_name);
    n
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
fn with_current_target(event: &web_sys::Event, current_target: &JsValue, action: impl FnOnce()) {
    let key = JsValue::from_str("currentTarget");
    let previous = js_sys::Reflect::get(event.as_ref(), &key).ok();

    define_event_property(event, &key, current_target);
    action();

    if let Some(previous) = previous {
        define_event_property(event, &key, &previous);
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
fn define_event_property(event: &web_sys::Event, key: &JsValue, value: &JsValue) {
    let descriptor = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&descriptor, &JsValue::from_str("configurable"), &JsValue::from_bool(true));
    let _ = js_sys::Reflect::set(&descriptor, &JsValue::from_str("value"), value);
    let _ = js_sys::Reflect::define_property(event.as_ref(), key, descriptor.as_ref());
}
