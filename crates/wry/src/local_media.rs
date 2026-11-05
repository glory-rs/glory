//! Explicit, document-local media surfaces. No URL or filesystem authority.
use crate::{Renderer, protocol};
use glory_render::{Error, NodeId, Props};
use serde_json::json;

#[derive(Clone, Copy, Debug)]
pub enum MediaKind {
    Audio,
    Image,
    Video,
}
pub struct LocalMedia<'a> {
    pub kind: MediaKind,
    /// An opaque exact path served by the host's explicit local asset provider.
    pub path: &'a str,
    pub title: &'a str,
}
pub(crate) fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= 1024
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        && !path.split('/').any(|v| v == "." || v == "..")
}
impl Renderer {
    /// Explicit opt-in before constructing the shell. Grants only host-served
    /// local media, never network URLs, file paths, script or autoplay.
    pub fn enable_local_media(&self) {
        self.0.borrow_mut().local_media = true;
    }
    pub fn set_local_media(
        &self,
        node: NodeId,
        media: Option<LocalMedia<'_>>,
    ) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        state.transport.check()?;
        if !state.local_media {
            return Err(Error::Unsupported(
                "Wry local media requires explicit host opt-in",
            ));
        }
        if !matches!(
            state.mapper.tree().node(node)?.descriptor.view().props,
            Props::View
        ) {
            return Err(Error::InvalidProps("Wry media requires a View"));
        }
        let value = if let Some(media) = media {
            if !valid_path(media.path) || media.title.len() > 4096 {
                return Err(Error::InvalidProps("Invalid local media source"));
            }
            json!({"kind":match media.kind {MediaKind::Audio=>"audio",MediaKind::Video=>"video",MediaKind::Image=>"img"},"path":media.path,"title":media.title})
        } else {
            serde_json::Value::Null
        };
        state
            .transport
            .push(json!({"op":"local_media","id":protocol::id(node),"value":value}))
    }
}
