//! Helpers for widgets that implement controlled input reconciliation.

use glory_render::NodeId;

use crate::{Error, Result, Scope};

impl Scope {
    /// Write a text input using the next revision of the live semantic node.
    /// Equal values still acknowledge host edits without resetting caret/preedit.
    /// Call after a host tick (for example in an owned Effect), so later events
    /// from that tick retain their original revision until routing completes.
    pub fn set_input_value(&self, node: NodeId, value: impl Into<String>) -> Result {
        self.session()?.input_value(node, value.into())
    }

    /// Reconcile a native select after a complete host tick using its live revision.
    pub fn set_select_selection(
        &self,
        node: NodeId,
        keys: Vec<glory_render::SemanticId>,
    ) -> Result {
        self.session()?.select_selection(node, keys)
    }

    /// Start a new image request revision, including an explicit same-URL retry.
    pub fn set_image_source(&self, node: NodeId, source: impl Into<String>) -> Result {
        self.session()?.image_source(node, source.into())
    }

    /// Report a custom widget's asynchronous Binding/Effect/handler error at the
    /// next App flush. Retired scopes ignore late errors without retaining App.
    pub fn report_error(&self, error: impl Into<Error>) {
        if let Ok(session) = self.session() {
            session.report(error.into());
        }
    }
}
