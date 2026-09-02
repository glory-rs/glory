use crate::service::notify::Watched;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// sent when a bin target source file is changed
    BinSource,
    /// sent when a lib target source file is changed
    LibSource,
    /// sent when an asset file changed
    Asset(Watched),
    /// sent when a style file changed
    Style,
    /// Cargo.toml changed
    Conf,
}

/// The set of source changes seen since the last consumed build.
///
/// `revision` is a monotonic counter bumped by every recorded filesystem
/// event, **including one that does not widen the set**. A build snapshots the
/// set together with its revision and may only clear it if the revision is
/// still the same; otherwise an edit landed while the build was running and
/// must not be dropped. Without this, saving the same file twice during one
/// build silently loses the second save.
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    changes: Vec<Change>,
    revision: u64,
}

impl ChangeSet {
    pub fn all_changes() -> Self {
        let mut set = Self::default();
        set.add_all();
        set
    }

    /// Widen the set to every change kind, as done on a rescan or a manual
    /// rebuild request.
    pub fn add_all(&mut self) {
        for change in [
            Change::BinSource,
            Change::LibSource,
            Change::Style,
            Change::Conf,
            Change::Asset(Watched::Rescan),
        ] {
            self.add(change);
        }
    }

    /// Monotonic counter of recorded filesystem events. See the type docs.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Drop the recorded changes. The revision is deliberately preserved so a
    /// later clear can still tell whether it is looking at the same generation.
    pub fn clear(&mut self) {
        self.changes.clear()
    }

    pub fn need_server_build(&self) -> bool {
        self.changes.contains(&Change::BinSource) || self.changes.contains(&Change::Conf)
    }

    pub fn need_front_build(&self) -> bool {
        self.changes.contains(&Change::LibSource) || self.changes.contains(&Change::Conf)
    }

    pub fn asset_iter(&self) -> impl Iterator<Item = &Watched> {
        self.changes.iter().filter_map(|change| match change {
            Change::Asset(a) => Some(a),
            _ => None,
        })
    }

    pub fn need_style_build(&self, css_files: bool, css_in_source: bool) -> bool {
        (css_files && self.changes.contains(&Change::Style)) || (css_in_source && self.changes.contains(&Change::LibSource))
    }

    /// Record a change. Returns whether it widened the set; the revision is
    /// bumped either way so a concurrent build can tell that something moved.
    pub fn add(&mut self, change: Change) -> bool {
        self.revision = self.revision.wrapping_add(1);
        if !self.changes.contains(&change) {
            self.changes.push(change);
            true
        } else {
            false
        }
    }
}
