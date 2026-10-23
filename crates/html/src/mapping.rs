use crate::*;
use glory_render::{Change, Descriptor, Error, FocusMode, Patch, Props, Purpose, Role, TextKind};
use std::collections::BTreeMap;

fn element(part: Part, parent: Option<Part>, tag: &'static str) -> Piece {
    Piece {
        part,
        parent,
        kind: Kind::Element {
            tag,
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
        },
    }
}
fn attrs(shape: &mut Shape, part: Part) -> &mut BTreeMap<&'static str, String> {
    let Kind::Element { attributes, .. } = &mut shape
        .pieces
        .iter_mut()
        .find(|piece| piece.part == part)
        .expect("element part")
        .kind
    else {
        unreachable!()
    };
    attributes
}
fn props(shape: &mut Shape, part: Part) -> &mut BTreeMap<&'static str, Property> {
    let Kind::Element { properties, .. } = &mut shape
        .pieces
        .iter_mut()
        .find(|piece| piece.part == part)
        .expect("element part")
        .kind
    else {
        unreachable!()
    };
    properties
}
fn text(shape: &mut Shape, value: &str) {
    shape.pieces.push(Piece {
        part: Part::Content,
        parent: Some(Part::Main),
        kind: Kind::Text(value.to_owned()),
    });
}
fn enabled(shape: &mut Shape, part: Part, enabled: bool) {
    attrs(shape, Part::Main).insert(
        if enabled {
            "data-gly-enabled"
        } else {
            "data-gly-disabled"
        },
        String::new(),
    );
    props(shape, part).insert("disabled", Property::Bool(!enabled));
    if !enabled {
        attrs(shape, part).insert("disabled", String::new());
    }
}
fn changed<T: Copy + Default>(old: T, change: &Change<T>) -> T {
    match change {
        Change::Keep => old,
        Change::Set(value) => *value,
        Change::Clear => T::default(),
    }
}
fn view_tag(role: Option<Role>) -> &'static str {
    match role {
        Some(Role::Main) => "main",
        Some(Role::Navigation) => "nav",
        Some(Role::Section) => "section",
        _ => "div",
    }
}
pub(crate) fn role_name(role: Role) -> &'static str {
    match role {
        Role::Group => "group",
        Role::Main => "main",
        Role::Navigation => "navigation",
        Role::Section => "region",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Status => "status",
        Role::Alert => "alert",
        Role::Dialog => "dialog",
    }
}
pub(crate) fn input_type(purpose: Purpose) -> &'static str {
    match purpose {
        Purpose::Password => "password",
        Purpose::Search => "search",
        Purpose::Tel => "tel",
        Purpose::Text | Purpose::Number | Purpose::Email | Purpose::Url => "text",
    }
}
pub(crate) fn input_mode(purpose: Purpose) -> Option<&'static str> {
    match purpose {
        Purpose::Number => Some("decimal"),
        Purpose::Email => Some("email"),
        Purpose::Url => Some("url"),
        _ => None,
    }
}
fn text_tag(kind: TextKind, options: Option<&HtmlTextOptions>) -> Result<&'static str, Error> {
    let default = match kind {
        TextKind::Inline => TextTag::Span,
        TextKind::Paragraph => TextTag::P,
        TextKind::Heading(level @ 1..=6) => [
            TextTag::H1,
            TextTag::H2,
            TextTag::H3,
            TextTag::H4,
            TextTag::H5,
            TextTag::H6,
        ][level as usize - 1],
        TextKind::Heading(_) => return Err(Error::InvalidProps("heading level must be 1..=6")),
    };
    let selected = options.and_then(|options| options.tag).unwrap_or(default);
    if (matches!(kind, TextKind::Heading(_) | TextKind::Paragraph) && selected != default)
        || matches!(kind, TextKind::Inline) && !matches!(selected, TextTag::Span | TextTag::P)
    {
        return Err(Error::InvalidProps(
            "HTML text tag conflicts with semantic kind",
        ));
    }
    Ok(match selected {
        TextTag::Span => "span",
        TextTag::P => "p",
        TextTag::H1 => "h1",
        TextTag::H2 => "h2",
        TextTag::H3 => "h3",
        TextTag::H4 => "h4",
        TextTag::H5 => "h5",
        TextTag::H6 => "h6",
    })
}

/// Check descriptor-local HTML constraints without shaping text, allocating
/// styles, reading a host tree or cloning a node's potentially large value.
/// Creation checks mapping options; patches preserve the existing HTML tag.
pub fn preflight(node: &Descriptor, patch: Option<&Patch>) -> Result<(), Error> {
    let tags = match (&node.props, patch) {
        (Props::Text(text), None) => {
            text_tag(text.kind, node.options.get::<HtmlTextOptions>())?;
            return Ok(());
        }
        (Props::Text(text), Some(Patch::TextKind(change))) => {
            let options = node.options.get::<HtmlTextOptions>();
            Some((
                text_tag(text.kind, options)?,
                text_tag(changed(text.kind, change), options)?,
            ))
        }
        (Props::View, Some(Patch::Role(change))) => {
            Some((view_tag(node.role), view_tag(changed(node.role, change))))
        }
        (Props::Input(input), Some(Patch::Multiline(change)))
            if input.multiline != changed(input.multiline, change) =>
        {
            return Err(Error::InvalidProps("HTML tag changes require remount"));
        }
        _ => None,
    };
    if tags.is_some_and(|(old, next)| old != next) {
        return Err(Error::InvalidProps("HTML tag changes require remount"));
    }
    Ok(())
}

pub fn shape(descriptor: &Descriptor) -> Result<Shape, Error> {
    descriptor.validate()?;
    if matches!(descriptor.props, Props::Group) {
        return Ok(Shape::default());
    }
    let tag = match &descriptor.props {
        Props::View => view_tag(descriptor.role),
        Props::Text(text) => text_tag(text.kind, descriptor.options.get::<HtmlTextOptions>())?,
        Props::Button(_) => "button",
        Props::RichText(_) => "div",
        Props::Input(input) if input.multiline => "textarea",
        Props::Input(_) => "input",
        Props::Check(_) => "label",
        Props::Image(_) => "img",
        Props::Link(_) => "a",
        Props::Group => unreachable!(),
    };
    let mut shape = Shape {
        pieces: vec![element(Part::Main, None, tag)],
        control: Some(Part::Main),
        children: descriptor.props.accepts_children().then_some(Part::Main),
        prevent_activate_default: descriptor
            .options
            .get::<HtmlEventOptions>()
            .is_some_and(|options| options.prevent_default),
        ..Shape::default()
    };
    let kind = match &descriptor.props {
        Props::View => "view",
        Props::Text(value) => {
            text(&mut shape, &value.content);
            if !value.selectable {
                attrs(&mut shape, Part::Main).insert("style", "user-select:none;".into());
            }
            "text"
        }
        Props::Button(value) => {
            attrs(&mut shape, Part::Main).insert("type", "button".into());
            text(&mut shape, &value.label);
            enabled(&mut shape, Part::Main, value.enabled);
            "button"
        }
        Props::RichText(value) => {
            shape.edit = EditKind::RichText;
            shape.revision = value.revision;
            let attributes = attrs(&mut shape, Part::Main);
            attributes.insert("role", "textbox".into());
            attributes.insert("aria-multiline", "true".into());
            attributes.insert(
                "contenteditable",
                (value.enabled && !value.read_only).to_string(),
            );
            attributes.insert("aria-readonly", value.read_only.to_string());
            attributes.insert("aria-disabled", (!value.enabled).to_string());
            attributes.insert(
                if value.enabled {
                    "data-gly-enabled"
                } else {
                    "data-gly-disabled"
                },
                String::new(),
            );
            attributes.insert("data-gly-placeholder", value.placeholder.clone());
            props(&mut shape, Part::Main).insert(
                "richDocument",
                Property::RichDocument(value.document.clone()),
            );
            "richtext"
        }
        Props::Input(value) => {
            shape.edit = EditKind::Text;
            shape.revision = value.revision;
            props(&mut shape, Part::Main).insert("value", Property::Text(value.value.clone()));
            if !value.multiline {
                attrs(&mut shape, Part::Main).insert("type", input_type(value.purpose).into());
                attrs(&mut shape, Part::Main).insert("value", value.value.clone());
                if let Some(mode) = input_mode(value.purpose) {
                    attrs(&mut shape, Part::Main).insert("inputmode", mode.into());
                }
            }
            enabled(&mut shape, Part::Main, value.enabled);
            "input"
        }
        Props::Check(value) => {
            shape.control = Some(Part::Control);
            shape.edit = EditKind::Check;
            shape
                .pieces
                .push(element(Part::Control, Some(Part::Main), "input"));
            attrs(&mut shape, Part::Control).insert("type", "checkbox".into());
            if value.checked {
                attrs(&mut shape, Part::Control).insert("checked", String::new());
                attrs(&mut shape, Part::Main).insert("data-gly-checked", String::new());
            }
            props(&mut shape, Part::Control).insert("checked", Property::Bool(value.checked));
            enabled(&mut shape, Part::Control, value.enabled);
            text(&mut shape, &value.label);
            "check"
        }
        Props::Image(value) => {
            if !value.source.is_empty() {
                attrs(&mut shape, Part::Main).insert("src", value.source.clone());
            }
            attrs(&mut shape, Part::Main)
                .insert("alt", value.alternative.clone().unwrap_or_default());
            if value.alternative.is_none() {
                attrs(&mut shape, Part::Main).insert("aria-hidden", "true".into());
            }
            "image"
        }
        Props::Link(value) => {
            attrs(&mut shape, Part::Main).insert("href", value.target.clone());
            text(&mut shape, &value.label);
            "link"
        }
        Props::Group => unreachable!(),
    };
    attrs(&mut shape, Part::Main).insert("data-gly-kind", kind.into());
    if let Some(part) = descriptor.options.get::<glory_render::VirtualPart>() {
        attrs(&mut shape, Part::Main).insert(
            "data-gly-virtual",
            match part {
                glory_render::VirtualPart::Root => "root",
                glory_render::VirtualPart::Surface => "surface",
                glory_render::VirtualPart::Row => "row",
            }
            .into(),
        );
    }
    if !descriptor.classes.is_empty() {
        attrs(&mut shape, Part::Main).insert("class", descriptor.classes.join(" "));
    }
    let css = style(&descriptor.style);
    if !css.is_empty() {
        attrs(&mut shape, Part::Main)
            .entry("style")
            .or_default()
            .push_str(&css);
    }
    let control = shape.control.expect("element control");
    if let Some(name) = &descriptor.name {
        attrs(&mut shape, control).insert("aria-label", name.clone());
    }
    if let Some(role) = descriptor.effective_role() {
        attrs(&mut shape, Part::Main).insert("role", role_name(role).into());
    }
    match descriptor.focus {
        FocusMode::Auto => {}
        FocusMode::Programmatic => {
            attrs(&mut shape, control).insert("tabindex", "-1".into());
        }
        FocusMode::TabStop => {
            attrs(&mut shape, control).insert("tabindex", "0".into());
        }
    }
    if let Some(item) = descriptor.item {
        attrs(&mut shape, Part::Main).insert("aria-posinset", (item.index + 1).to_string());
        attrs(&mut shape, Part::Main).insert(
            "aria-setsize",
            item.count
                .map_or_else(|| "-1".into(), |count| count.to_string()),
        );
    }
    for piece in &mut shape.pieces {
        match &mut piece.kind {
            Kind::Text(value) => *value = normalize(value),
            Kind::Element {
                attributes,
                properties,
                ..
            } => {
                for value in attributes.values_mut() {
                    *value = normalize(value);
                }
                for value in properties.values_mut() {
                    if let Property::Text(value) = value {
                        *value = normalize(value);
                    }
                }
            }
        }
    }
    Ok(shape)
}
