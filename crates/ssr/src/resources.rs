use glory_resource::{Error, Resources};
use glory_ui::Widget;

#[derive(Clone, Debug)]
pub enum SnapshotError {
    Ui(glory_ui::Error),
    Resources(Error),
}
impl From<glory_ui::Error> for SnapshotError {
    fn from(error: glory_ui::Error) -> Self {
        Self::Ui(error)
    }
}
impl From<Error> for SnapshotError {
    fn from(error: Error) -> Self {
        Self::Resources(error)
    }
}
impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SnapshotError {}

#[derive(Clone, Debug)]
pub struct ResourceSnapshot {
    pub view: crate::Snapshot,
    pub data: glory_resource::Snapshot,
}
/// Synchronous rendering of explicitly prefetched/seeded request data. Export
/// while the App is alive, before Owner cleanup removes named registrations.
/// This does not wait for futures or implement HTTP streaming.
pub fn snapshot_resources_with<W: Widget>(
    resources: &Resources,
    factory: impl FnOnce(&glory_ui::Scope) -> W,
) -> Result<ResourceSnapshot, SnapshotError> {
    let (_app, output) = crate::mount_with(factory)?;
    let body = output.snapshot().map_err(glory_ui::Error::from)?;
    Ok(ResourceSnapshot {
        view: crate::fragment(body),
        data: resources.snapshot()?,
    })
}

/// Inert data block. Place outside the exclusively hydrated mount; select by
/// its application-specific id. No executable inline patch/bootstrap script.
pub fn resource_script(
    id: &str,
    snapshot: &glory_resource::Snapshot,
    nonce: Option<&str>,
) -> Result<String, Error> {
    let json = snapshot.to_json()?;
    let data = glory_html::escape_script_data(&json);
    if data.len() > snapshot.limits().bytes {
        return Err(Error::Limit("script data bytes"));
    }
    let nonce = nonce
        .map(|nonce| format!(" nonce=\"{}\"", glory_html::escape(nonce)))
        .unwrap_or_default();
    Ok(format!(
        "<script id=\"{}\" type=\"application/json\" data-gly-resources=\"{}\"{nonce}>{}</script>",
        glory_html::escape(id),
        glory_resource::VERSION,
        data
    ))
}
