use crate::{Location, Navigation, Result, RouterHandle};

/// Compose the existing portable Link. Its real href works before WASM; the
/// semantic request marker suppresses only an ordinary activation's default.
/// The application explicitly receives blocked/failed navigation results.
pub fn link(
    router: RouterHandle,
    location: Location,
    mut on_result: impl FnMut(Result<Navigation>) + 'static,
) -> glory_ui::Link {
    glory_ui::link()
        .target(location.to_string())
        .semantics(Some(Box::new(glory_render::Semantics {
            link_request: true,
            ..Default::default()
        })))
        .on(glory_ui::events::activate, move |_| {
            on_result(router.push(location.clone()))
        })
}
