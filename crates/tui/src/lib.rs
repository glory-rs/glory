//! # glory-tui — command-stream demo backend
//!
//! **Status: demo / debugging aid, not a product target.** (Dioxus retired
//! its TUI backend; Glory deliberately does not invest here.) What this
//! crate offers is a read-only ratatui view over the in-memory
//! [`CommandDom`] reference interpreter — useful for inspecting what a
//! widget tree emits without opening a webview.

pub use glory_core::renderer::command_dom::{CommandDom, DomNode, ROOT_ID};
pub use glory_core::renderer::{Command as TuiCommand, CommandNode as TuiNode, CommandRenderer as TuiRenderer, EventData as TuiEventPayload};
pub use ratatui;

/// Renders the current `CommandDom` tree as indented lines, one node per
/// line — the simplest possible read-only "frame" for a ratatui paragraph.
pub fn render_outline(dom: &CommandDom) -> String {
    let mut out = String::new();
    render_node(dom, ROOT_ID, 0, &mut out);
    out
}

fn render_node(dom: &CommandDom, id: u64, depth: usize, out: &mut String) {
    let Some(node) = dom.node(id) else { return };
    out.push_str(&"  ".repeat(depth));
    out.push('<');
    out.push_str(&node.name);
    out.push('>');
    if let Some(text) = &node.text {
        out.push(' ');
        out.push_str(text);
    }
    out.push('\n');
    for child in &node.children {
        render_node(dom, *child, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_core::renderer::Command;

    #[test]
    fn outline_renders_tree() {
        let mut dom = CommandDom::new();
        dom.apply_batch(&[
            Command::Create {
                id: 1,
                name: "div".into(),
                is_void: false,
            },
            Command::SetText { id: 1, value: "hi".into() },
            Command::Insert {
                parent: ROOT_ID,
                child: 1,
                position: glory_core::renderer::CommandInsertPosition::Tail,
            },
        ]);
        assert_eq!(render_outline(&dom), "<body>\n  <div> hi\n");
    }
}
