//! Resolve a request before rendering; retain real 404/3xx/HEAD semantics.
use glory_routing::{RequestHistory, RequestResolution, Router, RouterHandle, Routes};
use glory_ui::{Scope, Widget};
use salvo::{
    Response,
    http::{HeaderValue, StatusCode, header},
};
use std::rc::Rc;

pub use glory_routing::{Decision, RequestLocation};

/// Render a pre-resolved request with local, request-owned UI state. The caller
/// can run request guards through RequestLocation::resolve_with first.
pub fn render_response_with<W: Widget>(
    routes: Routes,
    request: RequestResolution,
    head: bool,
    factory: impl FnOnce(&Scope, RouterHandle) -> W + 'static,
) -> glory_ui::Result<Response> {
    let status = StatusCode::from_u16(request.status)
        .map_err(|error| glory_render::Error::Backend(error.to_string()))?;
    if let Some(target) = request.redirect {
        let mut response = Response::new();
        response.status_code(status);
        response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&target.to_string())
                .map_err(|error| glory_render::Error::Backend(error.to_string()))?,
        );
        return Ok(response);
    }
    if (status != StatusCode::OK && status != StatusCode::NOT_FOUND)
        || !matches!(request.error, None | Some(glory_routing::Error::NoMatch))
    {
        let mut response = crate::html_response(format!(
            "<!doctype html><title>{}</title><p>{}</p>",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Routing error")
        ));
        response.status_code(status);
        if head {
            response.take_body();
        }
        return Ok(response);
    }
    let mut factory = Some(factory);
    let html = glory_ssr::render(glory_ui::widget(move |scope| {
        let router = Router::new(
            scope,
            routes.clone(),
            Rc::new(RequestHistory::new(request.location.clone())),
        )
        .map_err(|error| glory_render::Error::Backend(error.to_string()))?;
        let factory = factory.take().ok_or(glory_ui::Error::Gone)?;
        factory(scope, router.handle()).show_in(scope)
    }))?;
    let mut response = crate::html_response(html);
    response.status_code(status);
    if head {
        response.take_body();
    }
    Ok(response)
}
