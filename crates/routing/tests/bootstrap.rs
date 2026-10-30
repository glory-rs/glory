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

#[test]
fn declaration_identity_includes_each_nodes_consumed_path_and_parameter_ownership() {
    const ROOT: RouteKey<Params> = RouteKey::new("root");
    const PAGE: RouteKey<Params> = RouteKey::new("page");
    let server = Routes::builder()
        .root(RouteDef::layout(ROOT, "").child(RouteDef::new(PAGE, ":id")))
        .finish()
        .unwrap();
    let client = Routes::builder()
        .root(RouteDef::layout(ROOT, ":id").child(RouteDef::new(PAGE, "")))
        .finish()
        .unwrap();
    let location = Location::parse("/42").unwrap();
    assert!(
        server.resolve(&location).unwrap().chain[0]
            .params
            .0
            .is_empty()
    );
    assert_eq!(
        client.resolve(&location).unwrap().chain[0]
            .params
            .required::<u32>("id"),
        Ok(42)
    );
    assert_ne!(server.schema("codec-v1"), client.schema("codec-v1"));
    let seed = Bootstrap::new(&server, location, "codec-v1").unwrap();
    assert_eq!(
        seed.validate(&client, "codec-v1"),
        Err(Error::HydrationMismatch)
    );
    let payload = seed.to_payload();
    assert!(payload.starts_with("glory-routing/2\n"));
    assert!(matches!(
        Bootstrap::from_payload(&payload.replacen("glory-routing/2", "glory-routing/1", 1)),
        Err(Error::HydrationMismatch)
    ));
    assert_eq!(
        Bootstrap::from_payload(&payload)
            .unwrap()
            .validate(&server, "codec-v1"),
        seed.validate(&server, "codec-v1")
    );
}
