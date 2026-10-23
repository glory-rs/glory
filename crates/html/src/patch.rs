//! Lower one validated field, not two whole Shapes. Creation/snapshots retain
//! full shaping. The inverse comes from Memory's rollback guard.
use crate::{Edit, Part, Property};
use glory_render::{
    Change, Descriptor, FocusMode, ItemPosition, Patch, Props, Role, normalize_text,
};

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

pub(crate) fn lower(next: &Descriptor, inverse: &Patch) -> Vec<Edit> {
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
            let new = inline_style(&next.style, selectable);
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
            let old = inline_style(&next.style, old);
            let new = inline_style(&next.style, value.selectable);
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
            attribute(
                &mut edits,
                control,
                "aria-label",
                old.as_deref(),
                next.name.as_deref(),
            );
            if matches!(next.props, Props::View) {
                attribute(
                    &mut edits,
                    Part::Main,
                    "role",
                    role(next.role, old.is_some()),
                    next.effective_role().map(crate::mapping::role_name),
                );
            }
        }
        Patch::Role(change) => {
            let old = *old!(change);
            if matches!(next.props, Props::View) {
                attribute(
                    &mut edits,
                    Part::Main,
                    "role",
                    role(old, next.name.is_some()),
                    next.effective_role().map(crate::mapping::role_name),
                );
            }
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
        Patch::Checked(change) => {
            let old = *old!(change);
            let Props::Check(value) = &next.props else {
                unreachable!()
            };
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
                edits.push(Edit::Property {
                    part: control,
                    name: "checked",
                    value: Property::Bool(value.checked),
                });
            }
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
        Patch::Source(change) => {
            let old = old!(change);
            let Props::Image(image) = &next.props else {
                unreachable!()
            };
            attribute(
                &mut edits,
                Part::Main,
                "src",
                nonempty(old),
                nonempty(&image.source),
            );
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
