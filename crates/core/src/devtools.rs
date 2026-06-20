//! Lightweight diagnostic snapshots for devtools and tests.
//!
//! These types are intentionally read-only. They expose enough runtime state to
//! build an inspector without letting inspector code mutate scheduler internals.

use serde::{Deserialize, Serialize};

pub const DEVTOOLS_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactiveKind {
    Cage,
    Bond,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReactiveSnapshot {
    pub kind: ReactiveKind,
    pub id: u64,
    pub version: usize,
    pub subscriber_count: usize,
    pub subscriber_views: Vec<String>,
    pub dependency_ids: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandQueueSnapshot {
    pub buffered_command_count: usize,
    pub handler_count: usize,
    pub pending_query_count: usize,
    pub next_node_id: u64,
    pub next_query_token: u64,
    pub coalesce: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DevtoolsSnapshot {
    pub protocol_version: u32,
    pub reactives: Vec<ReactiveSnapshot>,
    pub command_queues: Vec<CommandQueueSnapshot>,
}

impl Default for DevtoolsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl DevtoolsSnapshot {
    pub fn new() -> Self {
        Self {
            protocol_version: DEVTOOLS_PROTOCOL_VERSION,
            reactives: Vec::new(),
            command_queues: Vec::new(),
        }
    }

    pub fn with_reactive(mut self, snapshot: ReactiveSnapshot) -> Self {
        self.reactives.push(snapshot);
        self
    }

    pub fn with_command_queue(mut self, snapshot: CommandQueueSnapshot) -> Self {
        self.command_queues.push(snapshot);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum DevtoolsMessage {
    Hello { protocol_version: u32 },
    Snapshot(DevtoolsSnapshot),
    ReactiveSnapshot(ReactiveSnapshot),
    CommandQueueSnapshot(CommandQueueSnapshot),
    CommandBatch { commands: Vec<crate::renderer::Command> },
    Warning { message: String },
}

impl DevtoolsMessage {
    pub fn hello() -> Self {
        Self::Hello {
            protocol_version: DEVTOOLS_PROTOCOL_VERSION,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

/// Render a self-contained static inspector panel from a snapshot.
///
/// Hosts can serve or embed this HTML while richer live devtools are still
/// being wired up over [`DevtoolsMessage`].
pub fn render_snapshot_panel(snapshot: &DevtoolsSnapshot) -> String {
    let mut html = String::from(
        r#"<!doctype html><meta charset="utf-8"><title>Glory Devtools</title><style>
body{font:13px system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;margin:0;color:#1f2933;background:#f7f8fa}
main{padding:16px;max-width:1120px;margin:0 auto}
h1{font-size:20px;margin:0 0 16px}
h2{font-size:14px;margin:20px 0 8px}
table{width:100%;border-collapse:collapse;background:white;border:1px solid #d9dee7}
th,td{text-align:left;padding:8px;border-bottom:1px solid #e6eaf0;vertical-align:top}
th{font-size:12px;text-transform:uppercase;color:#536273;background:#eef2f6}
code{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}
.empty{color:#6b7785;background:white;border:1px solid #d9dee7;padding:12px}
</style><main><h1>Glory Devtools</h1>"#,
    );
    html.push_str("<h2>Reactive Graph</h2>");
    if snapshot.reactives.is_empty() {
        html.push_str(r#"<div class="empty">No reactive snapshots.</div>"#);
    } else {
        html.push_str(
            "<table><thead><tr><th>Kind</th><th>ID</th><th>Version</th><th>Subscribers</th><th>Views</th><th>Dependencies</th></tr></thead><tbody>",
        );
        for reactive in &snapshot.reactives {
            html.push_str("<tr><td>");
            html.push_str(match reactive.kind {
                ReactiveKind::Cage => "cage",
                ReactiveKind::Bond => "bond",
            });
            html.push_str("</td><td><code>");
            html.push_str(&reactive.id.to_string());
            html.push_str("</code></td><td>");
            html.push_str(&reactive.version.to_string());
            html.push_str("</td><td>");
            html.push_str(&reactive.subscriber_count.to_string());
            html.push_str("</td><td>");
            html.push_str(&escape_html(&reactive.subscriber_views.join(", ")));
            html.push_str("</td><td>");
            let dependencies = reactive.dependency_ids.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
            html.push_str(&escape_html(&dependencies));
            html.push_str("</td></tr>");
        }
        html.push_str("</tbody></table>");
    }

    html.push_str("<h2>Command Queues</h2>");
    if snapshot.command_queues.is_empty() {
        html.push_str(r#"<div class="empty">No command queue snapshots.</div>"#);
    } else {
        html.push_str("<table><thead><tr><th>Buffered</th><th>Handlers</th><th>Pending Queries</th><th>Next Node</th><th>Next Query</th><th>Coalesce</th></tr></thead><tbody>");
        for queue in &snapshot.command_queues {
            html.push_str("<tr><td>");
            html.push_str(&queue.buffered_command_count.to_string());
            html.push_str("</td><td>");
            html.push_str(&queue.handler_count.to_string());
            html.push_str("</td><td>");
            html.push_str(&queue.pending_query_count.to_string());
            html.push_str("</td><td>");
            html.push_str(&queue.next_node_id.to_string());
            html.push_str("</td><td>");
            html.push_str(&queue.next_query_token.to_string());
            html.push_str("</td><td>");
            html.push_str(if queue.coalesce { "true" } else { "false" });
            html.push_str("</td></tr>");
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</main>");
    html
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::Command;

    #[test]
    fn devtools_message_round_trips_json() {
        let message = DevtoolsMessage::Snapshot(DevtoolsSnapshot::new().with_reactive(ReactiveSnapshot {
            kind: ReactiveKind::Bond,
            id: 9,
            version: 2,
            subscriber_count: 1,
            subscriber_views: vec!["root-0".into()],
            dependency_ids: vec![7, 8],
        }));
        let json = message.to_json().unwrap();
        assert!(json.contains(r#""type":"snapshot""#));
        assert_eq!(DevtoolsMessage::from_json(&json).unwrap(), message);
    }

    #[test]
    fn devtools_protocol_carries_command_batches() {
        let message = DevtoolsMessage::CommandBatch {
            commands: vec![Command::SetText {
                id: 1,
                value: "hello".into(),
            }],
        };
        let json = message.to_json().unwrap();
        assert_eq!(DevtoolsMessage::from_json(&json).unwrap(), message);
    }

    #[test]
    fn snapshot_panel_renders_reactive_graph_and_escapes_text() {
        let html = render_snapshot_panel(
            &DevtoolsSnapshot::new()
                .with_reactive(ReactiveSnapshot {
                    kind: ReactiveKind::Cage,
                    id: 1,
                    version: 3,
                    subscriber_count: 1,
                    subscriber_views: vec!["<root>".into()],
                    dependency_ids: Vec::new(),
                })
                .with_command_queue(CommandQueueSnapshot {
                    buffered_command_count: 2,
                    handler_count: 1,
                    pending_query_count: 0,
                    next_node_id: 7,
                    next_query_token: 0,
                    coalesce: false,
                }),
        );
        assert!(html.contains("Glory Devtools"));
        assert!(html.contains("&lt;root&gt;"));
        assert!(html.contains("Reactive Graph"));
        assert!(html.contains("Command Queues"));
    }
}
