use crate::*;
use glory_render::{Change, Error, FocusMode, Patch, Props, Purpose, Role, TextKind};
use std::collections::BTreeMap;

fn normalize_in_place(value: &mut String) {
    if let std::borrow::Cow::Owned(normalized) = glory_render::normalize_text(value, true) {
        *value = normalized;
    }
}

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
fn view_tag(
    role: Option<Role>,
    part: Option<glory_render::FieldPart>,
    document: Option<glory_render::DocumentPart>,
) -> &'static str {
    if let Some(document) = document {
        return match document {
            glory_render::DocumentPart::Blockquote => "blockquote",
            glory_render::DocumentPart::OrderedList => "ol",
            glory_render::DocumentPart::UnorderedList => "ul",
            glory_render::DocumentPart::ListItem => "li",
            glory_render::DocumentPart::DescriptionList => "dl",
            glory_render::DocumentPart::Term => "dt",
            glory_render::DocumentPart::Definition => "dd",
            glory_render::DocumentPart::Table => "table",
            glory_render::DocumentPart::TableCaption => "caption",
            glory_render::DocumentPart::TableHead => "thead",
            glory_render::DocumentPart::TableBody => "tbody",
            glory_render::DocumentPart::TableFoot => "tfoot",
            glory_render::DocumentPart::TableRow => "tr",
            glory_render::DocumentPart::ColumnHeader | glory_render::DocumentPart::RowHeader => {
                "th"
            }
            glory_render::DocumentPart::TableCell => "td",
        };
    }
    if let Some(part) = part {
        return match part {
            glory_render::FieldPart::Form => "form",
            glory_render::FieldPart::Label => "label",
            glory_render::FieldPart::Fieldset => "fieldset",
            glory_render::FieldPart::Legend => "legend",
        };
    }
    match role {
        Some(Role::Main) => "main",
        Some(Role::Banner) => "header",
        Some(Role::ContentInfo) => "footer",
        Some(Role::Navigation) => "nav",
        Some(Role::Section) => "section",
        _ => "div",
    }
}
pub(crate) fn role_name(role: Role) -> &'static str {
    match role {
        Role::ProgressBar => "progressbar",
        Role::Slider => "slider",
        Role::SpinButton => "spinbutton",
        Role::Separator => "separator",
        Role::Group => "group",
        Role::Main => "main",
        Role::Banner => "banner",
        Role::ContentInfo => "contentinfo",
        Role::Navigation => "navigation",
        Role::Section => "region",
        Role::List => "list",
        Role::ListItem => "listitem",
        Role::Status => "status",
        Role::Alert => "alert",
        Role::Table => "table",
        Role::Grid => "grid",
        Role::GridCell => "gridcell",
        Role::Row => "row",
        Role::ColumnHeader => "columnheader",
        Role::Cell => "cell",
        Role::Dialog => "dialog",
        Role::AlertDialog => "alertdialog",
        Role::Tooltip => "tooltip",
        Role::TabList => "tablist",
        Role::TabPanel => "tabpanel",
        Role::RadioGroup => "radiogroup",
        Role::Toolbar => "toolbar",
        Role::Menu => "menu",
        Role::Menubar => "menubar",
        Role::Listbox => "listbox",
        Role::Heading(_) => "heading",
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
        TextKind::Code => TextTag::Code,
        TextKind::Kbd => TextTag::Kbd,
        TextKind::Mark => TextTag::Mark,
        TextKind::Paragraph => TextTag::P,
        TextKind::Heading(level @ 1..=6) => [
            TextTag::H1,
            TextTag::H2,
            TextTag::H3,
            TextTag::H4,
            TextTag::H5,
            TextTag::H6,
        ][level as usize - 1],
        TextKind::Heading(_) => {
            return Err(Error::InvalidProps("heading level must be 1..=6"));
        }
    };
    let selected = options.and_then(|options| options.tag).unwrap_or(default);
    if (kind != TextKind::Inline && selected != default)
        || matches!(kind, TextKind::Inline) && !matches!(selected, TextTag::Span | TextTag::P)
    {
        return Err(Error::InvalidProps(
            "HTML text tag conflicts with semantic kind",
        ));
    }
    Ok(match selected {
        TextTag::Span => "span",
        TextTag::Code => "code",
        TextTag::Kbd => "kbd",
        TextTag::Mark => "mark",
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
pub fn preflight<'a>(
    node: impl Into<glory_render::DescriptorView<'a>>,
    patch: Option<&Patch>,
) -> Result<(), Error> {
    let node = node.into();
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
        (Props::View, Some(Patch::Role(change))) => Some((
            view_tag(node.role, node.field_part(), node.document_part()),
            view_tag(
                changed(node.role, change),
                node.field_part(),
                node.document_part(),
            ),
        )),
        (Props::View, Some(Patch::Semantics(change))) => {
            let document = match change {
                Change::Keep => node.document_part(),
                Change::Clear => None,
                Change::Set(value) => value.as_ref().and_then(|s| s.document_part),
            };
            let part = match change {
                Change::Keep => node.field_part(),
                Change::Clear => None,
                Change::Set(value) => value.as_ref().and_then(|s| s.field_part),
            };
            Some((
                view_tag(node.role, node.field_part(), node.document_part()),
                view_tag(node.role, part, document),
            ))
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

pub fn shape<'a>(descriptor: impl Into<glory_render::DescriptorView<'a>>) -> Result<Shape, Error> {
    let descriptor = descriptor.into();
    descriptor.validate()?;
    if matches!(descriptor.props, Props::Group) {
        return Ok(Shape::default());
    }
    let tag = match &descriptor.props {
        Props::View => view_tag(
            descriptor.role,
            descriptor.field_part(),
            descriptor.document_part(),
        ),
        Props::Text(text) => text_tag(text.kind, descriptor.options.get::<HtmlTextOptions>())?,
        Props::Button(_) => "button",
        Props::Input(input) if input.multiline => "textarea",
        Props::Input(_) => "input",
        Props::Select(_) => "select",
        Props::Check(_) => "label",
        Props::Image(_) => "img",
        Props::Graphic(_) => "svg",
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
        Props::Select(value) => {
            shape.edit = EditKind::Select;
            shape.revision = value.selection.revision;
            shape.select_options = Some(crate::SelectOptions {
                entries: value.entries.clone(),
                placeholder: (!value.multiple).then(|| value.placeholder.clone()),
            });
            props(&mut shape, Part::Main).insert(
                "selectedKeys",
                Property::SelectedKeys(
                    value
                        .selection
                        .keys
                        .iter()
                        .map(|key| key.as_str().to_owned())
                        .collect(),
                ),
            );
            if value.multiple {
                attrs(&mut shape, Part::Main).insert("multiple", String::new());
            }
            if let Some(rows) = value.rows {
                attrs(&mut shape, Part::Main).insert("size", rows.to_string());
            }
            enabled(&mut shape, Part::Main, value.enabled);
            "select"
        }
        Props::Check(value) => {
            shape.control = Some(Part::Control);
            shape.edit = EditKind::Check;
            shape
                .pieces
                .push(element(Part::Control, Some(Part::Main), "input"));
            attrs(&mut shape, Part::Control).insert("type", check_type(value.mode).into());
            if value.checked {
                attrs(&mut shape, Part::Control).insert("checked", String::new());
                attrs(&mut shape, Part::Main).insert("data-gly-checked", String::new());
            }
            props(&mut shape, Part::Control).insert("checked", Property::Bool(value.checked));
            props(&mut shape, Part::Control).insert(
                "indeterminate",
                Property::Bool(value.mode == glory_render::CheckMode::Mixed),
            );
            if value.mode == glory_render::CheckMode::Mixed {
                attrs(&mut shape, Part::Control).insert("data-gly-indeterminate", String::new());
                attrs(&mut shape, Part::Main).insert("data-gly-mixed", String::new());
            }
            if value.mode == glory_render::CheckMode::Switch {
                attrs(&mut shape, Part::Control).insert("role", "switch".into());
            }
            attrs(&mut shape, Part::Control)
                .insert("aria-checked", check_aria(value.mode, value.checked).into());
            enabled(&mut shape, Part::Control, value.enabled);
            text(&mut shape, &value.label);
            "check"
        }
        Props::Image(value) => {
            shape.revision = value.revision;
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
        Props::Graphic(value) => {
            attrs(&mut shape, Part::Main)
                .extend(crate::graphic::main(value, descriptor.name.as_deref()));
            shape
                .pieces
                .push(element(Part::Content, Some(Part::Main), "path"));
            attrs(&mut shape, Part::Content).extend(crate::graphic::content(value));
            "graphic"
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
    let mut css = style(descriptor.style);
    css.push_str(&crate::semantics::translation_style(
        descriptor.semantics.as_deref().and_then(|s| s.translation),
    ));
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
    if let Some(Role::Heading(level)) = descriptor.role {
        attrs(&mut shape, Part::Main).insert("aria-level", level.to_string());
    }
    attrs(&mut shape, control).extend(crate::semantics::attributes(
        descriptor.semantics.as_deref(),
        descriptor.effective_role(),
    ));
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
            Kind::Text(value) => normalize_in_place(value),
            Kind::Element {
                attributes,
                properties,
                ..
            } => {
                for value in attributes.values_mut() {
                    normalize_in_place(value);
                }
                for value in properties.values_mut() {
                    if let Property::Text(value) = value {
                        normalize_in_place(value);
                    }
                }
            }
        }
    }
    Ok(shape)
}

/// Shared with field lowering: explicit false remains observable to AT.
pub(crate) fn check_aria(mode: glory_render::CheckMode, checked: bool) -> &'static str {
    if mode == glory_render::CheckMode::Mixed {
        "mixed"
    } else if checked {
        "true"
    } else {
        "false"
    }
}

pub(crate) fn check_type(mode: glory_render::CheckMode) -> &'static str {
    if mode == glory_render::CheckMode::Radio {
        "radio"
    } else {
        "checkbox"
    }
}

#[cfg(test)]
mod normalization_tests {
    use super::normalize_in_place;

    #[test]
    fn canonical_allocations_are_kept_and_html_parser_aliases_are_repaired() {
        for original in ["row 123", "Unicode 🙂 末尾", "a\nb", ""] {
            let mut value = original.to_owned();
            let allocation = value.as_ptr();
            let capacity = value.capacity();
            normalize_in_place(&mut value);
            assert_eq!(value, original);
            assert_eq!(value.as_ptr(), allocation);
            assert_eq!(value.capacity(), capacity);
        }
        for (original, expected) in [("a\r\nb\rc\0🙂", "a\nb\nc�🙂"), ("\0\r", "�\n")] {
            let mut value = original.to_owned();
            normalize_in_place(&mut value);
            assert_eq!(value, expected);
        }
    }
}
