//! Document-owned, shallow native prototypes. No labels, classes, URLs, values,
//! identities or other application data enter this bounded table.
#[cfg(target_arch = "wasm32")]
use super::super::backend;
#[cfg(target_arch = "wasm32")]
use glory_render::Error;
use std::collections::BTreeMap;
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, collections::HashMap};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element};

#[cfg(any(target_arch = "wasm32", test))]
#[path = "create.rs"]
mod create;

#[cfg(target_arch = "wasm32")]
const CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    tag: u8,
    kind: u8,
    role: u8,
    input_type: u8,
    state: u8,
}

// Copy only members of these finite vocabularies into keys. Even if lowering
// grows a new value, an unknown value takes the uncached path, never an owned
// String key or a cache of previously visited application values.
fn finite(value: Option<&String>, allowed: &[&str]) -> Option<u8> {
    match value {
        None => Some(0),
        Some(value) => allowed
            .iter()
            .position(|item| *item == value)
            .and_then(|index| u8::try_from(index + 1).ok()),
    }
}
impl Key {
    fn new(tag: &'static str, attrs: &BTreeMap<&'static str, String>) -> Option<Self> {
        let tag = [
            "div", "main", "nav", "section", "span", "p", "h1", "h2", "h3", "h4", "h5", "h6",
            "button", "input", "textarea", "label", "img", "a",
        ]
        .iter()
        .position(|item| *item == tag)? as u8;
        let kind = finite(
            attrs.get("data-gly-kind"),
            &["view", "text", "button", "input", "check", "image", "link"],
        )?;
        let role = finite(
            attrs.get("role"),
            &[
                "group",
                "main",
                "navigation",
                "region",
                "list",
                "listitem",
                "status",
                "alert",
                "table",
                "row",
                "columnheader",
                "cell",
                "dialog",
                "tablist",
                "tabpanel",
                "heading",
                "tab",
            ],
        )?;
        let input_type = finite(
            attrs.get("type"),
            &["button", "text", "password", "search", "tel", "checkbox"],
        )?;
        let mut state = 0;
        for (index, name) in ["data-gly-enabled", "data-gly-disabled", "disabled"]
            .into_iter()
            .enumerate()
        {
            if let Some(value) = attrs.get(name) {
                if !value.is_empty() {
                    return None;
                }
                state |= 1 << index;
            }
        }
        Some(Self {
            tag,
            kind,
            role,
            input_type,
            state,
        })
    }
}
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn inherited(name: &str) -> bool {
    matches!(
        name,
        "data-gly-kind" | "role" | "type" | "data-gly-enabled" | "data-gly-disabled" | "disabled"
    )
}

#[derive(Default)]
#[cfg(target_arch = "wasm32")]
pub(super) struct Templates(RefCell<HashMap<Key, Element>>);
#[cfg(target_arch = "wasm32")]
impl Templates {
    pub fn element(
        &self,
        document: &Document,
        tag: &'static str,
        attrs: &BTreeMap<&'static str, String>,
        token: &str,
        live: bool,
    ) -> Result<Element, Error> {
        let Some(key) = Key::new(tag, attrs) else {
            return initialize(
                document
                    .create_element_ns(Some(glory_html::element_namespace(tag)), tag)
                    .map_err(backend)?,
                attrs,
                false,
                token,
                live,
            );
        };
        let cached = self.0.borrow().get(&key).cloned();
        let template = if let Some(cached) = cached {
            cached
        } else {
            if self.0.borrow().len() >= CAPACITY {
                return initialize(
                    document.create_element(tag).map_err(backend)?,
                    attrs,
                    false,
                    token,
                    live,
                );
            }
            let template = document.create_element(tag).map_err(backend)?;
            for (name, value) in attrs {
                if inherited(name) {
                    template.set_attribute(name, value).map_err(backend)?;
                }
            }
            self.0.borrow_mut().insert(key, template.clone());
            template
        };
        if let Some(element) = create::element(&template, attrs, token, live)? {
            return Ok(element);
        }
        let element = template
            .clone_node()
            .map_err(backend)?
            .dyn_into::<Element>()
            .map_err(|value| backend(value.into()))?;
        initialize(element, attrs, true, token, live)
    }
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

#[cfg(target_arch = "wasm32")]
fn initialize(
    element: Element,
    attrs: &BTreeMap<&'static str, String>,
    cached: bool,
    token: &str,
    live: bool,
) -> Result<Element, Error> {
    for (name, value) in attrs {
        if !cached || !inherited(name) {
            element.set_attribute(name, value).map_err(backend)?;
        }
    }
    element
        .set_attribute("data-gly-node", token)
        .map_err(backend)?;
    if live {
        element
            .set_attribute("data-gly-live", "")
            .map_err(backend)?;
    }
    Ok(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_values_do_not_enter_the_key() {
        let mut attrs = BTreeMap::from([
            ("data-gly-kind", "input".into()),
            ("type", "password".into()),
        ]);
        let key = Key::new("input", &attrs).unwrap();
        for (name, value) in [
            ("class", "row-9284"),
            ("id", "business-id"),
            ("value", "secret"),
            ("aria-label", "private"),
            ("data-gly-node", "1.24.3"),
            ("checked", ""),
            ("style", "color:red"),
        ] {
            attrs.insert(name, value.into());
            assert_eq!(Key::new("input", &attrs), Some(key));
        }
        attrs.insert("type", "text".into());
        assert_ne!(Key::new("input", &attrs), Some(key));
        attrs.insert("disabled", String::new());
        let disabled = Key::new("input", &attrs).unwrap();
        attrs.remove("disabled");
        assert_ne!(Key::new("input", &attrs), Some(disabled));
    }

    #[test]
    fn unknown_vocabulary_and_nonempty_state_take_the_uncached_path() {
        assert!(Key::new("custom-element", &BTreeMap::new()).is_none());
        for (name, value) in [
            ("data-gly-kind", "future-kind"),
            ("role", "business-role"),
            ("type", "future-type"),
            ("data-gly-enabled", "true"),
            ("disabled", "disabled"),
        ] {
            assert!(Key::new("input", &BTreeMap::from([(name, value.into())])).is_none());
        }
    }
}
