use glory_routing::*;
#[test]
fn inert_bounded_seed_validates_schema_branch_and_fragment_independently_of_dom() {
    let routes = Routes::builder()
        .root(RouteDef::new(RouteKey::<Params>::new("page"), "page/:id"))
        .finish()
        .unwrap();
    let initial = Location::parse("/page/1?text=%3C%2Fscript%3E").unwrap();
    let seed = Bootstrap::new(&routes, initial.clone(), "codec-v1").unwrap();
    let payload = seed.to_payload();
    assert!(!payload.contains('<'));
    let restored = Bootstrap::from_payload(&payload).unwrap();
    assert_eq!(restored.validate(&routes, "codec-v1").unwrap(), initial);
    assert_eq!(
        restored.validate(&routes, "codec-v2"),
        Err(Error::HydrationMismatch)
    );
    assert!(Bootstrap::from_payload(&"x".repeat(65 * 1024)).is_err());
    let payload = payload.replace("\npage", "\nother");
    assert_eq!(
        Bootstrap::from_payload(&payload)
            .unwrap()
            .validate(&routes, "codec-v1"),
        Err(Error::HydrationMismatch)
    );
    let unknown =
        Bootstrap::new(&routes, Location::parse("/missing").unwrap(), "codec-v1").unwrap();
    assert_eq!(
        Bootstrap::from_payload(&unknown.to_payload())
            .unwrap()
            .validate(&routes, "codec-v1")
            .unwrap()
            .path(),
        "/missing"
    );
}
