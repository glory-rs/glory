//! Same display declarations as the XML branch, used only by the handwritten comparison.
use super::BODY;
use glory::display::*;
pub const CLASS: Field = Field::new("class");
pub const ID: Field = Field::new("id");
pub const LABEL: Field = Field::new("label");
pub const SELECT: Click = Click::new("select");
pub const REMOVE: Click = Click::new("remove");
pub const RUN: Click = Click::new("run");
pub const LOTS: Click = Click::new("lots");
pub const ADD: Click = Click::new("add");
pub const UPDATE: Click = Click::new("update");
pub const CLEAR: Click = Click::new("clear");
pub const SWAP: Click = Click::new("swap");
pub static ROW: Template = Template::new(&[div(
    &[
        attr("role", "row"),
        class("bench-row"),
        toggle_class("danger", CLASS),
    ],
    &[
        div(
            &[attr("role", "cell"), class("bench-cell col-md-1")],
            &[span(&[], &[field(ID)])],
        ),
        div(
            &[attr("role", "cell"), class("bench-cell col-md-4")],
            &[button(
                &[attr("type", "button"), class("lbl"), on_click(SELECT)],
                &[field(LABEL)],
            )],
        ),
        div(
            &[attr("role", "cell"), class("bench-cell col-md-1")],
            &[button(
                &[attr("type", "button"), class("remove"), on_click(REMOVE)],
                &[text("x")],
            )],
        ),
        div(&[attr("role", "cell"), class("bench-cell col-md-6")], &[]),
    ],
)]);
pub static PANEL: Template = Template::new(&[
    button(
        &[attr("type", "button"), class("bench-run"), on_click(RUN)],
        &[text("Create 1,000 rows")],
    ),
    button(
        &[
            attr("type", "button"),
            class("bench-runlots"),
            on_click(LOTS),
        ],
        &[text("Create 10,000 rows")],
    ),
    button(
        &[attr("type", "button"), class("bench-add"), on_click(ADD)],
        &[text("Append 1,000 rows")],
    ),
    button(
        &[
            attr("type", "button"),
            class("bench-update"),
            on_click(UPDATE),
        ],
        &[text("Update every 10th row")],
    ),
    button(
        &[
            attr("type", "button"),
            class("bench-clear"),
            on_click(CLEAR),
        ],
        &[text("Clear")],
    ),
    button(
        &[
            attr("type", "button"),
            class("bench-swaprows"),
            on_click(SWAP),
        ],
        &[text("Swap rows")],
    ),
    div(
        &[attr("role", "table"), class("bench-table")],
        &[div(&[class("bench-body")], &[outlet(BODY)])],
    ),
]);
