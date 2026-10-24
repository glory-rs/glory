//! Lower one validated field, not two whole Shapes. Creation/snapshots retain
//! full shaping. The inverse comes from Memory's rollback guard.
use crate::{Edit, Part, Property};
use glory_render::{Change, FocusMode, ItemPosition, Patch, Props, Role, normalize_text};

fn attribute(
    edits: &mut Vec<Edit>,
    part: Part,
    name: &'static str,
    old: Option<&str>,
    new: Option<&str>,
) {
    if old == new {
        return;
    }
    let old = old.map(|value| normalize_text(value, true));
    let new = new.map(|value| normalize_text(value, true));
    if old != new {
        edits.push(Edit::Attribute {
            part,
            name,
            value: new.map(|value| value.into_owned()),
        });
    }
}
fn text(edits: &mut Vec<Edit>, old: &str, new: &str) {
    if old == new {
        return;
    }
    let old = normalize_text(old, true);
    let new = normalize_text(new, true);
    if old != new {
        edits.push(Edit::Text {
            part: Part::Content,
            value: new.into_owned(),
        });
    }
}
fn inline_style(style: &glory_render::style::Style, selectable: bool) -> String {
    let mut value = if selectable {
        String::new()
    } else {
        "user-select:none;".to_owned()
    };
    value.push_str(&crate::style(style));
    value
}
fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
fn role(explicit: Option<Role>, named: bool) -> Option<&'static str> {
    explicit
        .or_else(|| named.then_some(Role::Group))
        .map(crate::mapping::role_name)
}
fn tabindex(focus: FocusMode) -> Option<&'static str> {
    match focus {
        FocusMode::Auto => None,
        FocusMode::Programmatic => Some("-1"),
        FocusMode::TabStop => Some("0"),
    }
}
fn position(item: Option<ItemPosition>) -> (Option<String>, Option<String>) {
    item.map_or((None, None), |item| {
        (
            Some((item.index + 1).to_string()),
            Some(
                item.count
                    .map_or_else(|| "-1".into(), |count| count.to_string()),
            ),
        )
    })
}

fn semantics(
    edits: &mut Vec<Edit>,
    part: Part,
    old: Option<&glory_render::Semantics>,
    new: Option<&glory_render::Semantics>,
    old_role: Option<Role>,
    new_role: Option<Role>,
) {
    let mut old = crate::semantics::attributes(old, old_role);
    let mut new = crate::semantics::attributes(new, new_role);
    if old.contains_key("type") || new.contains_key("type") {
        old.entry("type").or_insert_with(|| "button".into());
        new.entry("type").or_insert_with(|| "button".into());
    }
    for (name, value) in &new {
        attribute(
            edits,
            part,
            name,
            old.get(name).map(String::as_str),
            Some(value),
        );
    }
    for name in old.keys().filter(|name| !new.contains_key(*name)) {
        attribute(edits, part, name, old.get(name).map(String::as_str), None);
    }
}

pub(crate) fn lower(next: glory_render::DescriptorView<'_>, inverse: &Patch) -> Vec<Edit> {
    let mut edits = Vec::new();
    // Every accepted Group patch leaves its boxless shape empty.
    if matches!(next.props, Props::Group) {
        return edits;
    }
    let control = if matches!(next.props, Props::Check(_)) {
        Part::Control
    } else {
        Part::Main
    };
    macro_rules! old {
        ($change:expr) => {
            match $change {
                Change::Set(value) => value,
                Change::Keep => return edits,
                Change::Clear => unreachable!("inverse materializes defaults"),
            }
        };
    }
    match inverse {
        Patch::Text(change) => {
            let old = old!(change);
            let Props::Text(value) = &next.props else {
                unreachable!()
            };
            text(&mut edits, old, &value.content);
        }
        Patch::Label(change) => {
            let old = old!(change);
            let value = match &next.props {
                Props::Button(p) => &p.label,
                Props::Check(p) => &p.label,
                Props::Link(p) => &p.label,
                _ => unreachable!(),
            };
            text(&mut edits, old, value);
        }
        Patch::Classes(change) => {
            let old = old!(change).join(" ");
            let new = next.classes.join(" ");
            attribute(
                &mut edits,
                Part::Main,
                "class",
                nonempty(&old),
                nonempty(&new),
            );
        }
        Patch::Style(change) => {
            let old = old!(change);
            let selectable = !matches!(&next.props, Props::Text(text) if !text.selectable);
            let old = inline_style(old, selectable);
            let new = inline_style(next.style, selectable);
            attribute(
                &mut edits,
                Part::Main,
                "style",
                nonempty(&old),
                nonempty(&new),
            );
        }
        Patch::Selectable(change) => {
            let old = *old!(change);
            let Props::Text(value) = &next.props else {
                unreachable!()
            };
            let old = inline_style(next.style, old);
            let new = inline_style(next.style, value.selectable);
            attribute(
                &mut edits,
                Part::Main,
                "style",
                nonempty(&old),
                nonempty(&new),
            );
        }
        Patch::Name(change) => {
            let old = old!(change);
            let fallback = match next.props {
                Props::Graphic(graphic) => graphic.alternative.as_deref(),
                _ => None,
            };
            attribute(
                &mut edits,
                control,
                "aria-label",
                old.as_deref().or(fallback),
                next.name.as_deref().or(fallback),
            );
            if matches!(next.props, Props::View)
                && next.field_part() != Some(glory_render::FieldPart::Form)
                && next.document_part().is_none()
            {
                attribute(
                    &mut edits,
                    Part::Main,
                    "role",
                    role(
                        next.role,
                        old.is_some()
                            || next
                                .semantics
                                .as_ref()
                                .is_some_and(|s| s.labelled_by.is_some()),
                    ),
                    next.effective_role().map(crate::mapping::role_name),
                );
            }
        }
        Patch::Role(change) => {
            let old = *old!(change);
            let old = glory_render::DescriptorView { role: old, ..next }.effective_role();
            semantics(
                &mut edits,
                control,
                next.semantics.as_deref(),
                next.semantics.as_deref(),
                old,
                next.effective_role(),
            );
        }
        Patch::Semantics(change) => {
            let old = old!(change);
            let old_role = glory_render::DescriptorView {
                semantics: old,
                ..next
            }
            .effective_role();
            semantics(
                &mut edits,
                control,
                old.as_deref(),
                next.semantics.as_deref(),
                old_role,
                next.effective_role(),
            );
        }
        Patch::Focus(change) => {
            let old = *old!(change);
            attribute(
                &mut edits,
                control,
                "tabindex",
                tabindex(old),
                tabindex(next.focus),
            );
        }
        Patch::Item(change) => {
            let old = position(*old!(change));
            let new = position(next.item);
            attribute(
                &mut edits,
                Part::Main,
                "aria-posinset",
                old.0.as_deref(),
                new.0.as_deref(),
            );
            attribute(
                &mut edits,
                Part::Main,
                "aria-setsize",
                old.1.as_deref(),
                new.1.as_deref(),
            );
        }
        Patch::Enabled(change) => {
            let old = *old!(change);
            let new = next.props.enabled();
            if old != new {
                attribute(
                    &mut edits,
                    Part::Main,
                    "data-gly-enabled",
                    old.then_some(""),
                    new.then_some(""),
                );
                attribute(
                    &mut edits,
                    Part::Main,
                    "data-gly-disabled",
                    (!old).then_some(""),
                    (!new).then_some(""),
                );
                attribute(
                    &mut edits,
                    control,
                    "disabled",
                    (!old).then_some(""),
                    (!new).then_some(""),
                );
                edits.push(Edit::Property {
                    part: control,
                    name: "disabled",
                    value: Property::Bool(!new),
                });
            }
        }
        Patch::CheckMode(change) => {
            let old = *old!(change);
            let Props::Check(value) = &next.props else {
                unreachable!()
            };
            use glory_render::CheckMode;
            attribute(
                &mut edits,
                control,
                "type",
                Some(crate::mapping::check_type(old)),
                Some(crate::mapping::check_type(value.mode)),
            );
            if (old == CheckMode::Radio) != (value.mode == CheckMode::Radio) {
                edits.push(Edit::Property {
                    part: control,
                    name: "checked",
                    value: Property::Bool(value.checked),
                });
            }
            attribute(
                &mut edits,
                control,
                "role",
                (old == CheckMode::Switch).then_some("switch"),
                (value.mode == CheckMode::Switch).then_some("switch"),
            );
            attribute(
                &mut edits,
                control,
                "aria-checked",
                Some(crate::mapping::check_aria(old, value.checked)),
                Some(crate::mapping::check_aria(value.mode, value.checked)),
            );
            attribute(
                &mut edits,
                control,
                "data-gly-indeterminate",
                (old == CheckMode::Mixed).then_some(""),
                (value.mode == CheckMode::Mixed).then_some(""),
            );
            attribute(
                &mut edits,
                Part::Main,
                "data-gly-mixed",
                (old == CheckMode::Mixed).then_some(""),
                (value.mode == CheckMode::Mixed).then_some(""),
            );
            edits.push(Edit::Property {
                part: control,
                name: "indeterminate",
                value: Property::Bool(value.mode == CheckMode::Mixed),
            });
        }
        Patch::Checked(change) => {
            let old = *old!(change);
            let Props::Check(value) = &next.props else {
                unreachable!()
            };
            attribute(
                &mut edits,
                control,
                "aria-checked",
                Some(crate::mapping::check_aria(value.mode, old)),
                Some(crate::mapping::check_aria(value.mode, value.checked)),
            );
            if old != value.checked {
                attribute(
                    &mut edits,
                    Part::Main,
                    "data-gly-checked",
                    old.then_some(""),
                    value.checked.then_some(""),
                );
                attribute(
                    &mut edits,
                    control,
                    "checked",
                    old.then_some(""),
                    value.checked.then_some(""),
                );
            }
            // Native interaction can diverge from the committed model. An
            // explicit write must restore the control even when the semantic
            // value is unchanged; Keep returned above without touching it.
            edits.push(Edit::Property {
                part: control,
                name: "checked",
                value: Property::Bool(value.checked),
            });
        }
        Patch::SelectSelection { keys, revision } => {
            let _old = old!(keys);
            let Props::Select(select) = next.props else {
                unreachable!()
            };
            if *revision != select.selection.revision {
                edits.push(Edit::Revision(select.selection.revision));
            }
            // Explicit equal writes acknowledge/reject native changes as well.
            edits.push(Edit::Property {
                part: control,
                name: "selectedKeys",
                value: Property::SelectedKeys(
                    select
                        .selection
                        .keys
                        .iter()
                        .map(|key| key.as_str().to_owned())
                        .collect(),
                ),
            });
        }
        Patch::Value { value, revision } => {
            let old = old!(value);
            let Props::Input(input) = &next.props else {
                unreachable!()
            };
            if *revision != input.revision {
                edits.push(Edit::Revision(input.revision));
            }
            if !input.multiline {
                attribute(&mut edits, control, "value", Some(old), Some(&input.value));
            }
            if *revision != input.revision || *old != input.value {
                edits.push(Edit::Property {
                    part: control,
                    name: "value",
                    value: Property::Text(input.value.clone()),
                });
            }
        }
        Patch::Purpose(change) => {
            let old = *old!(change);
            let Props::Input(input) = &next.props else {
                unreachable!()
            };
            if !input.multiline {
                attribute(
                    &mut edits,
                    control,
                    "type",
                    Some(crate::mapping::input_type(old)),
                    Some(crate::mapping::input_type(input.purpose)),
                );
                attribute(
                    &mut edits,
                    control,
                    "inputmode",
                    crate::mapping::input_mode(old),
                    crate::mapping::input_mode(input.purpose),
                );
            }
        }
        Patch::Source { source, revision } => {
            let old = old!(source);
            let Props::Image(image) = &next.props else {
                unreachable!()
            };
            if *revision != image.revision {
                edits.push(Edit::Revision(image.revision));
            }
            let same_source_retry = *revision != image.revision
                && glory_render::normalize_text(old, true).as_ref() == image.source
                && !image.source.is_empty();
            attribute(
                &mut edits,
                Part::Main,
                "src",
                nonempty(old),
                nonempty(&image.source),
            );
            if same_source_retry {
                edits.push(Edit::Attribute {
                    part: Part::Main,
                    name: "src",
                    value: Some(image.source.clone()),
                });
            }
        }
        Patch::Alternative(change) => {
            let old = old!(change);
            let Props::Image(image) = &next.props else {
                unreachable!()
            };
            attribute(
                &mut edits,
                Part::Main,
                "alt",
                Some(old.as_deref().unwrap_or_default()),
                Some(image.alternative.as_deref().unwrap_or_default()),
            );
            attribute(
                &mut edits,
                Part::Main,
                "aria-hidden",
                old.is_none().then_some("true"),
                image.alternative.is_none().then_some("true"),
            );
        }
        Patch::Graphic(change) => {
            let old = old!(change);
            let Props::Graphic(graphic) = next.props else {
                unreachable!()
            };
            for (part, old, new) in [
                (
                    Part::Main,
                    crate::graphic::main(old, next.name.as_deref()),
                    crate::graphic::main(graphic, next.name.as_deref()),
                ),
                (
                    Part::Content,
                    crate::graphic::content(old),
                    crate::graphic::content(graphic),
                ),
            ] {
                for (name, value) in &new {
                    attribute(
                        &mut edits,
                        part,
                        name,
                        old.get(name).map(String::as_str),
                        Some(value),
                    );
                }
                for name in old.keys().filter(|name| !new.contains_key(*name)) {
                    attribute(
                        &mut edits,
                        part,
                        name,
                        old.get(name).map(String::as_str),
                        None,
                    );
                }
            }
        }
        Patch::Target(change) => {
            let old = old!(change);
            let Props::Link(link) = &next.props else {
                unreachable!()
            };
            attribute(
                &mut edits,
                Part::Main,
                "href",
                Some(old),
                Some(&link.target),
            );
        }
        // The adapter preflight already established that the physical tag stays.
        Patch::TextKind(_) | Patch::Multiline(_) => {}
    }
    // Match full Shape difference ordering: revision, then each piece's new
    // attributes, removals and properties. At most three edits for one field.
    edits.sort_by_key(|edit| match edit {
        Edit::Revision(_) => (0, Part::Main, 0, ""),
        Edit::Attribute { part, name, value } => (1, *part, u8::from(value.is_none()), *name),
        Edit::Property { part, name, .. } => (1, *part, 2, *name),
        Edit::Text { part, .. } => (1, *part, 2, ""),
    });
    edits
}
