//! A synchronous bridge for the common fresh-element fields. Names are JS
//! literals; values are borrowed for this call and never retained in a cache.
use std::collections::BTreeMap;

#[cfg(target_arch = "wasm32")]
use glory_render::Error;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
#[cfg(target_arch = "wasm32")]
use web_sys::Element;

#[cfg(target_arch = "wasm32")]
use super::super::backend;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "
export function gloryCloneElement(prototype, classes, style, token, live) {
    const element = prototype.cloneNode(false);
    if (classes != null) element.setAttribute('class', classes);
    if (style != null) element.setAttribute('style', style);
    element.setAttribute('data-gly-node', token);
    if (live) element.setAttribute('data-gly-live', '');
    return element;
}
")]
extern "C" {
    #[wasm_bindgen(catch, js_name = gloryCloneElement)]
    fn clone_element(
        prototype: &Element,
        classes: Option<&str>,
        style: Option<&str>,
        token: &str,
        live: bool,
    ) -> Result<Element, JsValue>;
}

fn eligible(attributes: &BTreeMap<&'static str, String>) -> bool {
    attributes
        .keys()
        .all(|name| super::inherited(name) || matches!(*name, "class" | "style"))
}

/// Other fields keep the generic ordered attribute path. In particular this
/// never folds value/checked writes or input composition into initialization.
#[cfg(target_arch = "wasm32")]
pub(super) fn element(
    prototype: &Element,
    attributes: &BTreeMap<&'static str, String>,
    token: &str,
    live: bool,
) -> Result<Option<Element>, Error> {
    if !eligible(attributes) {
        return Ok(None);
    }
    clone_element(
        prototype,
        attributes.get("class").map(String::as_str),
        attributes.get("style").map(String::as_str),
        token,
        live,
    )
    .map(Some)
    .map_err(backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_fields_use_the_generic_ordered_path() {
        let mut attributes = BTreeMap::from([
            ("class", "a🙂\n\"<>&\\\0".into()),
            ("style", String::new()),
            ("data-gly-kind", "input".into()),
            ("role", "group".into()),
        ]);
        assert!(eligible(&attributes));
        for name in [
            "aria-label",
            "value",
            "checked",
            "id",
            "data-gly-node",
            "x-unknown",
        ] {
            attributes.insert(name, String::new());
            assert!(!eligible(&attributes), "{name}");
            attributes.remove(name);
        }
        assert!(eligible(&BTreeMap::new()));
    }
}
