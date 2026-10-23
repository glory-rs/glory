//! Shared full-shape and field-local lowering for finite widget metadata.
use glory_render::{Role, Semantics};
use std::collections::BTreeMap;

pub(crate) fn attributes(
    value: Option<&Semantics>,
    role: Option<Role>,
) -> BTreeMap<&'static str, String> {
    let mut attrs = BTreeMap::new();
    if let Some(role) = role {
        attrs.insert("role", crate::mapping::role_name(role).into());
    }
    if let Some(Role::Heading(level)) = role {
        attrs.insert("aria-level", level.to_string());
    }
    let Some(value) = value else { return attrs };
    for (name, id) in [
        ("id", &value.id),
        ("aria-controls", &value.controls),
        ("aria-labelledby", &value.labelled_by),
        ("aria-describedby", &value.described_by),
        ("for", &value.label_for),
    ] {
        if let Some(id) = id {
            attrs.insert(name, id.as_str().to_owned());
        }
    }
    if let Some(error) = &value.error_message
        && value.invalid
    {
        attrs.insert("aria-errormessage", error.as_str().into());
        attrs
            .entry("aria-describedby")
            .and_modify(|description| {
                description.push(' ');
                description.push_str(error.as_str());
            })
            .or_insert_with(|| error.as_str().into());
    }
    for (flag, name) in [(value.required, "required"), (value.readonly, "readonly")] {
        if flag {
            attrs.insert(name, String::new());
        }
    }
    for (flag, name) in [
        (value.required, "aria-required"),
        (value.readonly, "aria-readonly"),
        (value.invalid, "aria-invalid"),
    ] {
        if flag {
            attrs.insert(name, "true".into());
        }
    }
    if value.readonly {
        attrs.insert("data-gly-readonly", String::new());
    }
    if value.invalid {
        attrs.insert("data-gly-invalid", String::new());
    }
    if value.field_part == Some(glory_render::FieldPart::Fieldset) && value.disabled {
        attrs.insert("disabled", String::new());
    }
    if value.tab {
        attrs.insert("role", "tab".into());
    }
    if value.has_popup {
        attrs.insert("aria-haspopup", "dialog".into());
    }
    if let Some(layer) = &value.layer {
        attrs.insert("popover", "manual".into());
        attrs.insert("data-gly-layer", layer.placement.as_str().into());
        attrs.insert("aria-modal", layer.modal.to_string());
    }
    for (name, state) in [
        ("aria-expanded", value.expanded),
        ("aria-selected", value.selected),
    ] {
        if let Some(state) = state {
            attrs.insert(name, state.to_string());
        }
    }
    if value.disabled {
        attrs.insert("aria-disabled", "true".into());
    }
    if value.expanded == Some(true) {
        attrs.insert("data-gly-expanded", String::new());
    }
    if value.selected == Some(true) {
        attrs.insert("data-gly-selected", String::new());
    }
    if value.hidden {
        attrs.insert("hidden", String::new());
        attrs.insert("data-gly-hidden", String::new());
    }
    if let Some(axis) = value.navigation {
        attrs.insert("data-gly-navigation", axis.as_str().into());
        if role == Some(Role::TabList) {
            attrs.insert("aria-orientation", axis.as_str().into());
        }
    }
    if value.navigation_item {
        attrs.insert("data-gly-navigation-item", String::new());
    }
    attrs
}
