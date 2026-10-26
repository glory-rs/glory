//! Shared full-shape and field-local lowering for finite widget metadata.
use glory_render::{Role, Semantics};
use std::borrow::Cow;
use std::collections::BTreeMap;

fn direction(value: glory_render::Direction) -> &'static str {
    match value {
        glory_render::Direction::Ltr => "ltr",
        glory_render::Direction::Rtl => "rtl",
    }
}

// These ARIA coordinates have the same wire rules for progress, spinbuttons,
// sliders and splitters. Missing coordinates stay absent; exact business text
// remains separate from the finite numeric coordinates.
fn range_attributes(
    attrs: &mut BTreeMap<&'static str, String>,
    min: Option<f64>,
    max: Option<f64>,
    current: Option<f64>,
    text: Option<&str>,
) {
    for (name, value) in [
        ("aria-valuemin", min),
        ("aria-valuemax", max),
        ("aria-valuenow", current),
    ] {
        if let Some(value) = value {
            attrs.insert(name, value.to_string());
        }
    }
    if let Some(text) = text {
        attrs.insert("aria-valuetext", text.to_owned());
    }
}

pub(crate) fn translation_style(motion: Option<glory_render::TranslationMotion>) -> String {
    motion.map_or_else(String::new, |motion| {
        let offset = motion.resting_offset();
        format!("transform:translate({}px,{}px);", offset.x, offset.y)
    })
}

pub(crate) const ROLE_ATTRIBUTE_COUNT: usize = 5;

/// Complete dependency set for a Role patch, in attribute-name order.
/// Read only the finite facets that can override these keys. In particular,
/// do not lower names, relations, range text, sheets or any other payload.
/// Values are canonical ASCII tokens or a generated integer, never user text.
pub(crate) fn role_attributes(
    value: Option<&Semantics>,
    role: Option<Role>,
) -> [(&'static str, Option<Cow<'static, str>>); ROLE_ATTRIBUTE_COUNT] {
    let mut name = role.map(crate::mapping::role_name);
    let mut level = None;
    let mut multiple = None;
    let mut modal = None;
    let mut orientation = None;
    if let Some(value) = value {
        if let Some(item) = value.tree_item {
            level = Some(item.level.to_string());
        }
        multiple = match role {
            Some(Role::Grid) => Some(value.grid_multiple),
            Some(Role::Listbox) => Some(value.listbox_multiple),
            _ => value.tree_navigation.map(|tree| tree.multiple),
        };
        orientation = value.separator_axis;
        if let Some(slider) = &value.slider {
            orientation = Some(slider.axis);
        }
        if matches!(
            role,
            Some(Role::TabList | Role::RadioGroup | Role::Toolbar | Role::Menubar)
        ) && let Some(axis) = value.navigation
        {
            orientation = Some(axis);
        }
        if matches!(role, Some(Role::Dialog | Role::AlertDialog)) {
            modal = value.layer.as_ref().map(|layer| layer.modal);
        }
        // Preserve the full mapper's intrinsic override precedence.
        if value.combobox.is_some() {
            name = Some("combobox");
        }
        if value.option.is_some() {
            name = Some("option");
        }
        if value.tab {
            name = Some("tab");
        }
        if let Some(item) = &value.menu_item {
            name = Some(match item.kind {
                glory_render::MenuItemKind::Action => "menuitem",
                glory_render::MenuItemKind::Checkbox => "menuitemcheckbox",
                glory_render::MenuItemKind::Radio => "menuitemradio",
            });
        }
    }
    if level.is_none()
        && let Some(Role::Heading(heading)) = role
    {
        level = Some(heading.to_string());
    }
    let flag = |value| Cow::Borrowed(if value { "true" } else { "false" });
    [
        ("aria-level", level.map(Cow::Owned)),
        ("aria-modal", modal.map(flag)),
        ("aria-multiselectable", multiple.map(flag)),
        (
            "aria-orientation",
            orientation.map(|axis| Cow::Borrowed(axis.as_str())),
        ),
        ("role", name.map(Cow::Borrowed)),
    ]
}

pub(crate) fn attributes(
    value: Option<&Semantics>,
    role: Option<Role>,
) -> BTreeMap<&'static str, String> {
    let mut attrs = BTreeMap::new();
    for (name, value) in role_attributes(value, role) {
        if let Some(value) = value {
            attrs.insert(name, value.into_owned());
        }
    }
    let Some(value) = value else { return attrs };
    if let Some(placeholder) = &value.placeholder {
        attrs.insert("placeholder", placeholder.clone());
    }
    if let Some(environment) = &value.environment {
        attrs.insert("dir", direction(environment.direction).into());
        attrs.insert("lang", environment.language.as_str().into());
    }
    if let Some(scope) = &value.focus_scope {
        attrs.insert("data-gly-focus-scope", scope.active.to_string());
        attrs.insert("data-gly-focus-loop", scope.loop_tab.to_string());
        attrs.insert("data-gly-focus-contain", scope.contain_focus.to_string());
        attrs.insert("data-gly-focus-auto", scope.auto_focus.to_string());
        attrs.insert("data-gly-focus-restore", scope.restore_focus.to_string());
        if let Some(id) = &scope.initial_focus {
            attrs.insert("data-gly-focus-initial", id.as_str().into());
        }
        if let Some(id) = &scope.return_focus {
            attrs.insert("data-gly-focus-return", id.as_str().into());
        }
    }
    if let Some(checked) = value.tree_checked {
        attrs.insert("aria-checked", checked.as_str().into());
    }
    if let Some(tree) = value.tree_navigation {
        attrs.insert("data-gly-tree-navigation", direction(tree.direction).into());
        attrs.insert("data-gly-tree-typeahead", tree.typeahead.to_string());
        attrs.insert("dir", direction(tree.direction).into());
    }
    if let Some(item) = value.tree_item {
        attrs.insert("aria-posinset", (u32::from(item.position) + 1).to_string());
        attrs.insert("aria-setsize", item.count.to_string());
        attrs.insert("data-gly-tree-item", String::new());
    }
    if let Some(capture) = value.swipe_capture {
        attrs.insert("data-gly-swipe-axis", capture.axis_name().into());
        attrs.insert("data-gly-swipe-revision", capture.revision.to_string());
    }
    if let Some(capture) = value.input_edit {
        attrs.insert("data-gly-input-edit", capture.max_bytes.to_string());
    }
    if let Some(translation) = value.translation {
        use glory_render::TranslationKind;
        let resting = translation.resting_offset();
        attrs.insert("data-gly-translation-x", format!("{}px", resting.x));
        attrs.insert("data-gly-translation-y", format!("{}px", resting.y));
        let (kind, from, to, millis) = match translation.kind {
            TranslationKind::Loop {
                from,
                to,
                period_millis,
            } => ("loop", from, to, period_millis),
            TranslationKind::To {
                target,
                duration_millis,
            } => ("to", target, target, u32::from(duration_millis)),
        };
        attrs.insert("data-gly-translation", kind.into());
        attrs.insert("data-gly-translation-from-x", from.x.to_string());
        attrs.insert("data-gly-translation-from-y", from.y.to_string());
        attrs.insert("data-gly-translation-to-x", to.x.to_string());
        attrs.insert("data-gly-translation-to-y", to.y.to_string());
        attrs.insert("data-gly-translation-duration", millis.to_string());
        attrs.insert(
            "data-gly-translation-paused",
            translation.paused.to_string(),
        );
    }
    match value.document_part {
        Some(glory_render::DocumentPart::ColumnHeader) => {
            attrs.insert("scope", "col".into());
        }
        Some(glory_render::DocumentPart::RowHeader) => {
            attrs.insert("scope", "row".into());
        }
        _ => {}
    }
    if let Some(sort) = value.sort {
        attrs.insert(
            "aria-sort",
            match sort {
                glory_render::SortDirection::None => "none",
                glory_render::SortDirection::Ascending => "ascending",
                glory_render::SortDirection::Descending => "descending",
            }
            .into(),
        );
    }
    if let Some(table) = &value.table {
        use glory_render::TableMetadata;
        match table {
            TableMetadata::Table { rows, columns } => {
                if let Some(rows) = rows {
                    attrs.insert("aria-rowcount", rows.to_string());
                }
                if let Some(columns) = columns {
                    attrs.insert("aria-colcount", columns.to_string());
                }
            }
            TableMetadata::Row { index } => {
                attrs.insert("aria-rowindex", (u64::from(*index) + 1).to_string());
            }
            TableMetadata::Cell {
                row,
                column,
                row_span,
                column_span,
                headers,
            } => {
                attrs.insert("aria-rowindex", (u64::from(*row) + 1).to_string());
                attrs.insert("aria-colindex", (u32::from(*column) + 1).to_string());
                attrs.insert(
                    if value.document_part.is_some() {
                        "rowspan"
                    } else {
                        "aria-rowspan"
                    },
                    row_span.to_string(),
                );
                attrs.insert(
                    if value.document_part.is_some() {
                        "colspan"
                    } else {
                        "aria-colspan"
                    },
                    column_span.to_string(),
                );
                if !headers.is_empty() {
                    attrs.insert(
                        "headers",
                        headers
                            .iter()
                            .map(|id| id.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
            }
        }
    }
    if let Some(assist) = value.input_assist {
        if let Some(autofill) = assist.autofill {
            attrs.insert("autocomplete", autofill.as_str().into());
        }
        if let Some(mode) = assist.mode {
            attrs.insert("inputmode", mode.as_str().into());
        }
    }
    if let Some(collapse) = value.collapse {
        attrs.insert(
            "data-gly-collapse",
            if collapse.open { "open" } else { "closed" }.into(),
        );
        attrs.insert(
            "data-gly-collapse-duration",
            format!("{}ms", collapse.duration_millis),
        );
        attrs.insert("data-gly-collapse-revision", collapse.revision.to_string());
        if !collapse.open {
            attrs.insert("aria-hidden", "true".into());
        }
    }
    if let Some(capture) = value.drag_capture {
        attrs.insert("data-gly-drag-capture", capture.as_str().into());
    }
    if let Some(policy) = value.tags_input {
        attrs.insert("data-gly-tags", policy.as_str().into());
    }
    if value.inert {
        attrs.insert("inert", String::new());
        attrs.insert("data-gly-inert", String::new());
    }
    if let Some(kind) = value.combobox {
        attrs.insert(
            "data-gly-combobox",
            match kind {
                glory_render::ComboboxKind::Select => "select",
                glory_render::ComboboxKind::Editable => "editable",
                glory_render::ComboboxKind::Autocomplete => "autocomplete",
                glory_render::ComboboxKind::Multiple => "multiple",
            }
            .into(),
        );
        if kind != glory_render::ComboboxKind::Select {
            attrs.insert("aria-autocomplete", "list".into());
        }
    }
    if let Some(option) = &value.option {
        attrs.insert("data-gly-option-owner", option.owner.as_str().into());
        attrs.insert("form", option.owner.as_str().into());
    }
    if let Some(progress) = &value.progress {
        range_attributes(
            &mut attrs,
            Some(progress.min),
            Some(progress.max),
            progress.current,
            progress.value_text.as_deref(),
        );
    }
    if let Some(busy) = value.busy {
        attrs.insert("aria-busy", busy.to_string());
        attrs.insert("data-gly-busy", busy.to_string());
    }
    if let Some(current) = value.current {
        attrs.insert(
            "aria-current",
            match current {
                glory_render::Current::Page => "page",
                glory_render::Current::Step => "step",
                glory_render::Current::Location => "location",
                glory_render::Current::Date => "date",
            }
            .into(),
        );
    }
    if let Some(spin) = &value.spinbutton {
        attrs.insert("data-gly-spinbutton", String::new());
        range_attributes(
            &mut attrs,
            spin.min,
            spin.max,
            spin.value,
            spin.value_text.as_deref(),
        );
        attrs.insert("data-gly-spin-increase", spin.can_increase.to_string());
        attrs.insert("data-gly-spin-decrease", spin.can_decrease.to_string());
    }
    if let Some(slider) = &value.slider {
        range_attributes(
            &mut attrs,
            Some(slider.range.min),
            Some(slider.range.max),
            Some(slider.range.value),
            slider.value_text.as_deref(),
        );
        attrs.insert("dir", direction(slider.direction).into());
        attrs.insert("data-gly-slider", String::new());
    }
    if let Some(range) = value.splitter {
        range_attributes(
            &mut attrs,
            Some(range.min),
            Some(range.max),
            Some(range.value),
            None,
        );
        attrs.insert("data-gly-splitter", String::new());
    }
    if let Some(axes) = value.scroll_keyboard {
        attrs.insert("data-gly-scroll-keyboard", axes.as_str().into());
    }
    if let Some(grid_direction) = value.grid_navigation {
        attrs.insert("data-gly-grid-navigation", direction(grid_direction).into());
    }
    if value.link_request {
        attrs.insert("data-gly-link-request", String::new());
    }
    if value.link_open == Some(glory_render::LinkOpen::NewContext) {
        attrs.insert("target", "_blank".into());
        attrs.insert("rel", "noopener noreferrer".into());
    }
    if value.field_part == Some(glory_render::FieldPart::Form) {
        attrs.insert("novalidate", String::new());
        attrs.insert("data-gly-form", String::new());
    }
    if let Some(action) = value.form_action {
        let action = match action {
            glory_render::FormAction::Submit => "submit",
            glory_render::FormAction::Reset => "reset",
        };
        attrs.insert("type", action.into());
        attrs.insert("data-gly-form-action", action.into());
    }
    if let Some(group) = &value.check_group {
        attrs.insert("name", group.as_str().into());
    }
    if let Some(control) = &value.form_control {
        // A custom range View is serialized by the explicit form snapshot API;
        // it is not silently presented as a native successful HTML input.
        let (name_key, value_key) = if value.slider.is_some() {
            ("data-gly-control-name", "data-gly-control-value")
        } else {
            ("name", "value")
        };
        attrs.insert(name_key, control.name.clone());
        if let Some(value) = &control.value {
            attrs.insert(value_key, value.clone());
        }
    }
    for (name, id) in [
        ("id", &value.id),
        ("aria-controls", &value.controls),
        ("aria-activedescendant", &value.active_descendant),
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
        if flag
            && value.slider.is_none()
            && (name != "required" || value.tags_input.is_none())
            && !matches!(
                (name, value.combobox),
                (_, Some(glory_render::ComboboxKind::Select))
                    | ("required", Some(glory_render::ComboboxKind::Multiple))
            )
        {
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
    if let Some(item) = &value.menu_item {
        attrs.insert("form", item.owner.as_str().into());
        attrs.insert("data-gly-menu-item", String::new());
    }
    if let Some(popup) = value.popup {
        attrs.insert("aria-haspopup", popup.as_str().into());
    }
    if value.sheet_grip {
        attrs.insert("data-gly-sheet-grip", String::new());
    }
    if let Some(layer) = &value.layer {
        if let glory_render::LayerPlacement::Viewport(placement) = layer.placement {
            attrs.insert("data-gly-viewport-x", placement.x.to_string());
            attrs.insert("data-gly-viewport-y", placement.y.to_string());
            attrs.insert(
                "data-gly-viewport-padding",
                placement.viewport_padding.to_string(),
            );
            attrs.insert(
                "data-gly-viewport-constrain",
                placement.constrain.to_string(),
            );
        }
        if let glory_render::LayerPlacement::Anchor(placement) = layer.placement {
            attrs.insert("data-gly-anchor-side", placement.side.as_str().into());
            attrs.insert("data-gly-anchor-align", placement.align.as_str().into());
            attrs.insert("data-gly-anchor-gap", placement.gap.to_string());
            attrs.insert(
                "data-gly-anchor-padding",
                placement.viewport_padding.to_string(),
            );
            attrs.insert("data-gly-anchor-flip", placement.flip.to_string());
            attrs.insert("data-gly-anchor-shift", placement.shift.to_string());
        }
        if let glory_render::LayerPlacement::ContextPoint(point) = layer.placement {
            attrs.insert("data-gly-context-x", point.x.to_string());
            attrs.insert("data-gly-context-y", point.y.to_string());
        }
        if let Some(sheet) = &layer.sheet {
            attrs.insert(
                "data-gly-sheet",
                sheet
                    .fractions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            attrs.insert("data-gly-sheet-current", sheet.current.to_string());
            attrs.insert("data-gly-sheet-max", sheet.max_extent.to_string());
            attrs.insert("data-gly-sheet-settle", sheet.settle_millis.to_string());
            attrs.insert("data-gly-sheet-dismissible", sheet.dismissible.to_string());
            attrs.insert("data-gly-sheet-revision", sheet.revision.to_string());
        }
        attrs.insert("popover", "manual".into());
        attrs.insert("data-gly-layer", layer.placement.as_str().into());
    }
    for (name, state) in [
        ("aria-expanded", value.expanded),
        ("aria-selected", value.selected),
        ("aria-pressed", value.pressed),
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
    if value.pressed == Some(true) {
        attrs.insert("data-gly-pressed", String::new());
    }
    if value.hidden {
        if value.collapse.is_none() {
            attrs.insert("hidden", String::new());
        }
        attrs.insert("data-gly-hidden", String::new());
    }
    if let Some(axis) = value.navigation {
        attrs.insert("data-gly-navigation", axis.as_str().into());
        attrs.insert("dir", direction(value.navigation_direction).into());
    }
    if value.typeahead {
        attrs.insert("data-gly-typeahead", String::new());
    }
    if let Some(policy) = value.context_menu {
        attrs.insert("data-gly-context", policy.as_str().into());
    }
    if value.menubar_item {
        attrs.insert("data-gly-menubar-item", String::new());
    }
    if value.navigation_item {
        attrs.insert("data-gly-navigation-item", String::new());
    }
    if let Some(panel) = value.navigation_panel {
        attrs.insert("data-gly-navigation-panel", panel.as_str().into());
    }
    attrs
}
