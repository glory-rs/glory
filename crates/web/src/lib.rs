//! Browser-only entry points are target gated; enabling web never changes core.
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "browser/templates.rs"]
mod template_tests;
#[cfg(target_arch = "wasm32")]
pub use browser::*;
#[cfg(feature = "stream")]
pub const STREAM_SCRIPT: &str = include_str!("stream.js");

/// Read a snapshot without modifying/consuming the data block. A failed
/// hydration can create a fresh Resources context from the same payload.
#[cfg(all(target_arch = "wasm32", feature = "resources"))]
pub fn read_resources(
    element: &web_sys::Element,
    limits: glory_resource::Limits,
) -> Result<glory_resource::Snapshot, glory_resource::Error> {
    use glory_resource::{Error, Snapshot, VERSION};
    if element.namespace_uri().as_deref() != Some("http://www.w3.org/1999/xhtml")
        || element.local_name() != "script"
        || element.get_attribute("type").as_deref() != Some("application/json")
    {
        return Err(Error::Payload(
            "expected an application/json script element",
        ));
    }
    if element.get_attribute("data-gly-resources").as_deref() != Some(VERSION.to_string().as_str())
    {
        return Err(Error::Payload("resource data block version"));
    }
    let json = element
        .text_content()
        .ok_or(Error::Payload("resource data block has no text"))?;
    Snapshot::from_json(&json, limits)
}

/// Restore explicit resources and adopt the same initial factory. Failure
/// preserves the payload for a fresh attempt; success removes its DOM data
/// block so large serialized values are not retained alongside live state.
#[cfg(all(target_arch = "wasm32", feature = "resources", feature = "hydrate"))]
pub fn hydrate_resources_with<W: glory_ui::Widget>(
    root: web_sys::Element,
    data: &web_sys::Element,
    limits: glory_resource::Limits,
    factory: impl FnOnce(&glory_ui::Scope, &glory_resource::Resources) -> W,
) -> Result<(WebApp, glory_resource::Resources), HydrationError> {
    if root.contains(Some(data.as_ref())) {
        return Err(HydrationError::Resources(glory_resource::Error::Payload(
            "resource block must be outside the mount",
        )));
    }
    let snapshot = read_resources(data, limits).map_err(HydrationError::Resources)?;
    let resources =
        glory_resource::Resources::restore(snapshot).map_err(HydrationError::Resources)?;
    let app = hydrate_with(root, |scope| factory(scope, &resources))?;
    data.remove();
    Ok((app, resources))
}
