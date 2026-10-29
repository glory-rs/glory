use glory_routing::*;
#[test]
fn request_resolution_distinguishes_not_found_bad_params_redirect_and_denial() {
    const PAGE: RouteKey<Params> = RouteKey::new("page");
    let routes = Routes::builder()
        .slash_policy(SlashPolicy::RedirectWithoutSlash)
        .root(RouteDef::new(PAGE, "page/:id"))
        .finish()
        .unwrap();
    assert_eq!(
        RequestLocation::parse("/page/1")
            .unwrap()
            .resolve(&routes)
            .status,
        200
    );
    assert_eq!(
        RequestLocation::parse("/missing")
            .unwrap()
            .resolve(&routes)
            .status,
        404
    );
    let redirect = RequestLocation::parse("/page/1/").unwrap().resolve(&routes);
    assert_eq!(redirect.status, 308);
    assert_eq!(redirect.redirect.unwrap().path(), "/page/1");
    let request = RequestLocation::parse("/page/1").unwrap();
    assert_eq!(
        request
            .resolve_with(&routes, |_, _| Ok(Decision::Denied(401)))
            .status,
        401
    );
    let redirected = request.resolve_with(&routes, |location, _| {
        if location.path() == "/page/1" {
            Ok(Decision::Redirect(Location::parse("/page/2").unwrap()))
        } else {
            Ok(Decision::Allow)
        }
    });
    assert_eq!(redirected.status, 302);
    assert_eq!(
        request
            .resolve_with(&routes, |location, _| Ok(Decision::Redirect(
                location.clone()
            )))
            .status,
        500
    );
    assert!(RequestLocation::parse("/page/1#fragment").is_err());
}
#[test]
fn immutable_request_history_never_performs_client_navigation() {
    let history = RequestHistory::new(Location::parse("/").unwrap());
    assert!(history.push(&Location::parse("/other").unwrap()).is_err());
    assert_eq!(history.current().unwrap().location.path(), "/");
}
