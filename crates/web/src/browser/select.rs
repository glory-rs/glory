//! Native select children and properties; no application source is inspected.
use glory_html::SelectOptions;
use glory_render::{Error, SelectEntry, SelectOption, SemanticId};
use wasm_bindgen::{JsCast, JsValue};
#[cfg(feature = "hydrate")]
use web_sys::Node;
use web_sys::{Document, Element};

use super::backend;

pub(super) fn reveal(dom: &super::dom::Dom, node: glory_render::NodeId) -> Result<(), Error> {
    let option = dom.entry(node)?.element()?;
    let Some(owner) = option.get_attribute("data-gly-option-owner") else {
        return Err(Error::Unsupported("option reveal target"));
    };
    let mut cursor = dom.parent(node);
    while let Some(id) = cursor {
        let entry = dom.entry(id)?;
        if let Ok(listbox) = entry.element()
            && listbox.get_attribute("role").as_deref() == Some("listbox")
        {
            if listbox.get_attribute("id").as_deref() != Some(owner.as_str())
                || !listbox.is_connected()
                || listbox.closest("[hidden]").map_err(backend)?.is_some()
            {
                return Ok(());
            }
            let bounds = listbox.get_bounding_client_rect();
            let item = option.get_bounding_client_rect();
            let top = bounds.top() + super::dom::number(&listbox, "clientTop");
            let bottom = top + super::dom::number(&listbox, "clientHeight");
            let delta = if item.top() < top {
                item.top() - top
            } else if item.bottom() > bottom {
                item.bottom() - bottom
            } else {
                0.0
            };
            if delta != 0.0 {
                let next = (super::dom::number(&listbox, "scrollTop") + delta).max(0.0);
                set(&listbox, "scrollTop", &next.into())?;
            }
            return Ok(());
        }
        cursor = dom.parent(id);
    }
    Ok(())
}

fn set(element: &Element, name: &str, value: &JsValue) -> Result<(), Error> {
    js_sys::Reflect::set(element, &name.into(), value).map_err(backend)?;
    Ok(())
}

pub fn write(element: &Element, keys: &[String]) -> Result<(), Error> {
    let options = options(element)?;
    for option in options {
        let selected = option
            .get_attribute("data-gly-option")
            .map_or(keys.is_empty(), |key| keys.contains(&key));
        if selected {
            option.set_attribute("selected", "").map_err(backend)?;
        } else {
            option.remove_attribute("selected").map_err(backend)?;
        }
        set(&option, "selected", &selected.into())?;
    }
    Ok(())
}

fn options(element: &Element) -> Result<Vec<Element>, Error> {
    let collection = js_sys::Reflect::get(element, &"options".into()).map_err(backend)?;
    let length = js_sys::Reflect::get(&collection, &"length".into())
        .map_err(backend)?
        .as_f64()
        .unwrap_or(f64::INFINITY);
    if !(0.0..=1025.0).contains(&length) {
        return Err(Error::Backend("native select option budget".into()));
    }
    js_sys::Array::from(&collection)
        .iter()
        .map(|value| value.dyn_into().map_err(backend))
        .collect()
}

pub fn read(element: &Element) -> Result<Vec<SemanticId>, Error> {
    let mut keys = Vec::new();
    for option in options(element)? {
        if js_sys::Reflect::get(&option, &"selected".into())
            .map_err(backend)?
            .as_bool()
            == Some(true)
            && let Some(key) = option.get_attribute("data-gly-option")
        {
            keys.push(SemanticId::new(key)?);
        }
    }
    Ok(keys)
}

pub fn create(
    document: &Document,
    parent: &Element,
    config: &SelectOptions,
    keys: &[String],
) -> Result<(), Error> {
    fn option(
        document: &Document,
        parent: &Element,
        value: &SelectOption,
        keys: &[String],
    ) -> Result<(), Error> {
        let node = document.create_element("option").map_err(backend)?;
        node.set_attribute("data-gly-option", value.key.as_str())
            .map_err(backend)?;
        node.set_attribute("value", &value.value).map_err(backend)?;
        if value.disabled {
            node.set_attribute("disabled", "").map_err(backend)?;
        }
        if keys.iter().any(|key| key == value.key.as_str()) {
            node.set_attribute("selected", "").map_err(backend)?;
        }
        node.set_text_content(Some(&value.label));
        parent.append_child(&node).map_err(backend)?;
        Ok(())
    }
    if let Some(placeholder) = &config.placeholder {
        let node = document.create_element("option").map_err(backend)?;
        for attr in ["data-gly-placeholder", "value", "disabled", "hidden"] {
            node.set_attribute(attr, "").map_err(backend)?;
        }
        if keys.is_empty() {
            node.set_attribute("selected", "").map_err(backend)?;
        }
        node.set_text_content(Some(placeholder));
        parent.append_child(&node).map_err(backend)?;
    }
    for entry in config.entries.iter() {
        match entry {
            SelectEntry::Option(value) => option(document, parent, value, keys)?,
            SelectEntry::Group(group) => {
                let node = document.create_element("optgroup").map_err(backend)?;
                node.set_attribute("label", &group.label).map_err(backend)?;
                if group.disabled {
                    node.set_attribute("disabled", "").map_err(backend)?;
                }
                for value in &group.options {
                    option(document, &node, value, keys)?;
                }
                parent.append_child(&node).map_err(backend)?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "hydrate")]
pub fn validate_children(
    document: &Document,
    element: &Element,
    config: &SelectOptions,
    keys: &[String],
) -> Result<bool, Error> {
    let expected = document.create_element("select").map_err(backend)?;
    create(document, &expected, config, keys)?;
    let mut actual = element.first_child();
    let mut candidate = expected.first_child();
    while let (Some(left), Some(right)) = (&actual, &candidate) {
        if !Node::is_equal_node(left, Some(right)) {
            return Ok(false);
        }
        actual = left.next_sibling();
        candidate = right.next_sibling();
    }
    Ok(actual.is_none() && candidate.is_none())
}
