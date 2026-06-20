//! Typed, builder-style `<head>` document components.
//!
//! These are thin, type-checked wrappers over the generated element factories
//! ([`title`], [`meta`], [`link`], [`script`]) that produce widgets suitable
//! for [`HeadMixin::fill`](super::HeadMixin::fill). On top of the raw factories
//! they add *deduplication* semantics that mirror Dioxus' `document` package:
//!
//! * Declaring `<title>` multiple times keeps only the **last** one.
//! * `<meta>` elements keyed by `name`/`property`/`charset`/`viewport` collapse
//!   to the **last** declaration for that key.
//!
//! The dedup logic itself is a pure, unit-testable fold over keyed entries
//! ([`dedup_head_entries`]); [`DedupHead`] applies it while still rendering
//! through the ordinary [`HeadMixin`](super::HeadMixin) pipeline so the SSR /
//! CSR / hydration paths stay identical to hand-written head fillers.

use std::borrow::Cow;

use crate::widget::{Filler, IntoFiller};
use crate::{Scope, Widget};

use super::head_mixin::HeadMixin;
use super::{link as link_el, meta as meta_el, script as script_el, title as title_el};

/// The category of a head element, used together with [`HeadKey::key`] to
/// decide which declarations collapse onto one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadKind {
    /// `<title>` — only one survives regardless of key.
    Title,
    /// `<meta>` — deduplicated per `name`/`property`/`charset`/`viewport`.
    Meta,
    /// `<link>` — deduplicated per `(rel, href)`.
    Link,
    /// `<script src=...>` — deduplicated per `src`.
    Script,
}

/// The full dedup key for a head entry: its [`HeadKind`] plus a discriminator
/// string (e.g. the meta `name`, or the link `href`). Entries sharing the same
/// `(kind, key)` collapse, the later one winning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeadKey {
    /// Element category.
    pub kind: HeadKind,
    /// Per-kind discriminator. Empty for singletons such as `<title>`.
    pub key: Cow<'static, str>,
}

impl HeadKey {
    fn new(kind: HeadKind, key: impl Into<Cow<'static, str>>) -> Self {
        Self { kind, key: key.into() }
    }
}

/// A single typed head declaration: its dedup key plus the deferred render.
///
/// Constructed via the free functions in this module ([`title`](self::title),
/// [`meta_name`], …) and consumed by [`DedupHead`] / [`HeadMixin::deduped`].
pub struct HeadEntry {
    key: HeadKey,
    filler: Filler,
}

impl HeadEntry {
    /// Builds a keyed entry from a dedup key and any fillable widget.
    pub fn new(kind: HeadKind, key: impl Into<Cow<'static, str>>, widget: impl IntoFiller) -> Self {
        Self {
            key: HeadKey::new(kind, key),
            filler: widget.into_filler(),
        }
    }

    /// The dedup key for this entry.
    pub fn key(&self) -> &HeadKey {
        &self.key
    }

    /// Consumes the entry, yielding its filler.
    pub fn into_filler(self) -> Filler {
        self.filler
    }
}

impl std::fmt::Debug for HeadEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeadEntry").field("key", &self.key).finish_non_exhaustive()
    }
}

// ----------------------------------------------------------------------------
// Pure dedup logic (unit-testable, no rendering side effects).
// ----------------------------------------------------------------------------

/// Folds head entries by `(kind, key)`, keeping the **last** declaration for
/// each key while preserving the relative order of first appearance.
///
/// This is the single source of truth for the dedup semantics and is exercised
/// directly by unit tests without touching the renderer.
pub fn dedup_head_entries(entries: Vec<HeadEntry>) -> Vec<HeadEntry> {
    // Index of the kept entry for each key, so a later duplicate replaces the
    // earlier one *in place* (stable order by first appearance).
    let mut slots: Vec<HeadEntry> = Vec::with_capacity(entries.len());
    let mut positions: Vec<HeadKey> = Vec::with_capacity(entries.len());

    for entry in entries {
        if let Some(idx) = positions.iter().position(|k| *k == entry.key) {
            slots[idx] = entry;
        } else {
            positions.push(entry.key.clone());
            slots.push(entry);
        }
    }

    slots
}

// ----------------------------------------------------------------------------
// Typed constructors.
// ----------------------------------------------------------------------------

/// `<title>text</title>`. Repeated titles dedupe to the last one.
pub fn title(text: impl Into<String>) -> HeadEntry {
    let text = text.into();
    HeadEntry::new(HeadKind::Title, "", title_el().text(text))
}

/// `<meta name=... content=...>`. Deduped per `name`.
pub fn meta_name(name: impl Into<Cow<'static, str>>, content: impl Into<String>) -> HeadEntry {
    let name = name.into();
    let content = content.into();
    HeadEntry::new(
        HeadKind::Meta,
        format!("name={name}"),
        meta_el().attr("name", name.into_owned()).attr("content", content),
    )
}

/// `<meta property=... content=...>` (Open Graph & friends). Deduped per
/// `property`.
pub fn meta_property(property: impl Into<Cow<'static, str>>, content: impl Into<String>) -> HeadEntry {
    let property = property.into();
    let content = content.into();
    HeadEntry::new(
        HeadKind::Meta,
        format!("property={property}"),
        meta_el().attr("property", property.into_owned()).attr("content", content),
    )
}

/// `<meta charset=...>`. There can be only one; deduped to the last.
pub fn charset(charset: impl Into<String>) -> HeadEntry {
    HeadEntry::new(HeadKind::Meta, "charset", meta_el().attr("charset", charset.into()))
}

/// `<meta name="viewport" content=...>`. Deduped to the last.
pub fn viewport(content: impl Into<String>) -> HeadEntry {
    meta_name("viewport", content)
}

/// `<link rel="stylesheet" href=...>`. Deduped per `href`.
pub fn stylesheet(href: impl Into<String>) -> HeadEntry {
    let href = href.into();
    HeadEntry::new(
        HeadKind::Link,
        format!("stylesheet:{href}"),
        link_el().attr("rel", "stylesheet").attr("href", href),
    )
}

/// `<script src=...></script>`. Deduped per `src`.
pub fn script_src(src: impl Into<String>) -> HeadEntry {
    let src = src.into();
    HeadEntry::new(HeadKind::Script, src.clone(), script_el().attr("src", src))
}

// ----------------------------------------------------------------------------
// DedupHead widget — applies dedup then renders through HeadMixin.
// ----------------------------------------------------------------------------

/// A [`HeadMixin`] convenience that collects typed [`HeadEntry`] values and
/// renders the deduplicated set into `<head>`.
///
/// ```ignore
/// use glory_core::web::widgets::document::{DedupHead, title, charset, viewport, meta_name};
///
/// DedupHead::new()
///     .with(charset("utf-8"))
///     .with(viewport("width=device-width, initial-scale=1"))
///     .with(title("Home"))
///     .with(meta_name("description", "first"))
///     .with(meta_name("description", "second")) // wins
///     .with(title("Final"))                      // wins
///     .show_in(ctx);
/// ```
#[derive(Debug, Default)]
pub struct DedupHead {
    entries: Vec<HeadEntry>,
}

impl DedupHead {
    /// Creates an empty head collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a typed head entry. Later entries with the same dedup key win.
    pub fn with(mut self, entry: HeadEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Builds the underlying [`HeadMixin`] with the deduplicated entries.
    pub fn into_mixin(self) -> HeadMixin {
        let mut mixin = HeadMixin::new();
        for entry in dedup_head_entries(self.entries) {
            mixin.fillers.push(entry.into_filler());
        }
        mixin
    }
}

impl Widget for DedupHead {
    // `DedupHead` is purely a builder front-end: it folds its entries and then
    // defers to a real [`HeadMixin`] view so the SSR / CSR / hydration paths are
    // byte-for-byte identical to a hand-written `head_mixin().fill(..)` chain.
    fn store_in(self, parent: &mut Scope) -> crate::ViewId {
        self.into_mixin().store_in(parent)
    }

    fn show_in(self, parent: &mut Scope) -> crate::ViewId {
        self.into_mixin().show_in(parent)
    }

    fn fill_in(self, parent: &mut Scope) {
        self.into_mixin().fill_in(parent)
    }

    fn build(&mut self, ctx: &mut Scope) {
        // Reached only if someone holds a `DedupHead` as a trait object and
        // drives the lifecycle manually; fold and render in place.
        let entries = std::mem::take(&mut self.entries);
        let mut mixin = HeadMixin::new();
        for entry in dedup_head_entries(entries) {
            mixin.fillers.push(entry.into_filler());
        }
        mixin.build(ctx);
    }

    fn flood(&mut self, ctx: &mut Scope) {
        self.patch(ctx);
    }
}

/// Shortcut for [`DedupHead::new`].
pub fn dedup_head() -> DedupHead {
    DedupHead::new()
}

impl HeadMixin {
    /// Fills this head with a deduplicated set of typed [`HeadEntry`] values.
    /// Existing fillers are kept; the deduped entries are appended.
    pub fn deduped(mut self, entries: Vec<HeadEntry>) -> Self {
        for entry in dedup_head_entries(entries) {
            self.fillers.push(entry.into_filler());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(entries: &[HeadEntry]) -> Vec<HeadKey> {
        entries.iter().map(|e| e.key().clone()).collect()
    }

    #[test]
    fn dedup_keeps_last_title_only() {
        let out = dedup_head_entries(vec![title("First"), title("Second"), title("Third")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].key().kind, HeadKind::Title);
    }

    #[test]
    fn dedup_collapses_same_meta_name() {
        let out = dedup_head_entries(vec![meta_name("description", "old"), meta_name("description", "new")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].key(), &HeadKey::new(HeadKind::Meta, "name=description"));
    }

    #[test]
    fn dedup_distinguishes_name_from_property() {
        let out = dedup_head_entries(vec![meta_name("title", "a"), meta_property("title", "b")]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn dedup_charset_and_viewport_are_singletons() {
        let out = dedup_head_entries(vec![
            charset("ascii"),
            viewport("width=1"),
            charset("utf-8"),
            viewport("width=device-width"),
        ]);
        // charset key + viewport (name=viewport) key => 2 survivors.
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn dedup_preserves_first_appearance_order() {
        let out = dedup_head_entries(vec![
            charset("utf-8"),
            title("A"),
            meta_name("description", "x"),
            title("B"),                    // collapses onto the earlier title slot
            meta_name("description", "y"), // collapses onto earlier meta slot
        ]);
        assert_eq!(
            keys(&out),
            vec![
                HeadKey::new(HeadKind::Meta, "charset"),
                HeadKey::new(HeadKind::Title, ""),
                HeadKey::new(HeadKind::Meta, "name=description"),
            ]
        );
    }

    #[test]
    fn dedup_link_and_script_keyed_by_href_src() {
        let out = dedup_head_entries(vec![
            stylesheet("/a.css"),
            stylesheet("/b.css"),
            stylesheet("/a.css"), // collapses onto the first /a.css
            script_src("/app.js"),
            script_src("/app.js"), // collapses
        ]);
        assert_eq!(keys(&out).len(), 3);
    }
}
