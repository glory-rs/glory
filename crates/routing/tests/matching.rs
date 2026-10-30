use glory_routing::*;

#[test]
fn full_size_route_tables_use_path_candidates_with_the_original_step_budget() {
    // Route identities are static declarations. This bounded test allocation is
    // shared by both tables rather than a production cache of visited URLs.
    let names = (0..4096)
        .map(|index| &*Box::leak(format!("page{index}").into_boxed_str()))
        .collect::<Vec<&'static str>>();
    for prefix in ["", ":locale/"] {
        let mut builder = Routes::builder().limits(Limits {
            match_steps: 8,
            ..Limits::default()
        });
        for (index, name) in names.iter().enumerate() {
            builder = builder.root(RouteDef::new(
                RouteKey::<Params>::new(name),
                format!("{prefix}section{index}/:id"),
            ));
        }
        let routes = builder.finish().unwrap();
        let head = if prefix.is_empty() { "" } else { "/en" };
        for index in [0, 2048, 4095] {
            let plan = routes
                .resolve(&Location::parse(&format!("{head}/section{index}/42")).unwrap())
                .unwrap();
            assert_eq!(plan.chain[0].id.0, names[index]);
            assert_eq!(plan.chain[0].params.required::<u32>("id"), Ok(42));
            assert_eq!(
                plan.chain[0].params.optional::<String>("locale").unwrap(),
                (!prefix.is_empty()).then(|| "en".into())
            );
        }
        assert!(matches!(
            routes.resolve(&Location::parse(&format!("{head}/missing/42")).unwrap()),
            Err(Error::NoMatch)
        ));
    }
}

#[test]
fn indexed_optional_branches_keep_greedy_captures_and_static_param_rest_priority() {
    const OPTIONAL: RouteKey<Params> = RouteKey::new("optional");
    const PAGE: RouteKey<Params> = RouteKey::new("page");
    const FIXED: RouteKey<Params> = RouteKey::new("fixed");
    const PARAM: RouteKey<Params> = RouteKey::new("param");
    const REST_KEY: RouteKey<Params> = RouteKey::new("rest-indexed");
    let routes = Routes::builder()
        .root(RouteDef::layout(OPTIONAL, ":locale?").child(RouteDef::new(PAGE, "docs/:id?")))
        .root(RouteDef::new(FIXED, "docs/new"))
        .root(RouteDef::new(PARAM, "docs/:id"))
        .root(RouteDef::new(REST_KEY, "*rest"))
        .finish()
        .unwrap();
    for (path, expected) in [
        ("/docs", PAGE.id()),
        ("/en/docs", PAGE.id()),
        ("/en/docs/42", PAGE.id()),
        ("/docs/new", FIXED.id()),
        ("/docs/42", PARAM.id()),
        ("/missing/a", REST_KEY.id()),
    ] {
        let plan = routes.resolve(&Location::parse(path).unwrap()).unwrap();
        assert_eq!(plan.chain.last().unwrap().id, expected, "{path}");
    }
    let plan = routes
        .resolve(&Location::parse("/en/docs/42").unwrap())
        .unwrap();
    assert_eq!(
        plan.params(OPTIONAL)
            .unwrap()
            .required::<String>("locale")
            .unwrap(),
        "en"
    );
    assert_eq!(plan.params(PAGE).unwrap().required::<u32>("id"), Ok(42));
    let plain = routes.resolve(&Location::parse("/docs").unwrap()).unwrap();
    assert_eq!(
        plain.params(OPTIONAL).unwrap().optional::<String>("locale"),
        Ok(None)
    );
}

const ROOT: RouteKey<()> = RouteKey::new("root");
const HOME: RouteKey<()> = RouteKey::new("home");
const USER: RouteKey<User> = RouteKey::new("user");
const NEW: RouteKey<()> = RouteKey::new("new");
const REST: RouteKey<Params> = RouteKey::new("rest");
#[derive(Debug, PartialEq)]
struct User {
    id: u64,
}
impl RouteParams for User {
    fn decode(params: &Params) -> Result<Self> {
        Ok(Self {
            id: params.required("id")?,
        })
    }
    fn encode(&self) -> Result<Params> {
        let mut params = Params::default();
        params.insert("id", self.id);
        Ok(params)
    }
}
fn routes() -> Routes {
    Routes::builder()
        .root(
            RouteDef::layout(ROOT, "")
                .child(RouteDef::new(HOME, ""))
                .child(RouteDef::new(REST, "*rest"))
                .child(RouteDef::new(USER, "users/:id"))
                .child(RouteDef::new(NEW, "users/new")),
        )
        .finish()
        .unwrap()
}
#[test]
fn static_priority_typed_targets_and_parent_parameter_isolation() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Routes>();
    let routes = routes();
    let matched = routes
        .resolve(&Location::parse("/users/new").unwrap())
        .unwrap();
    assert_eq!(matched.chain.last().unwrap().id, NEW.id());
    let target = routes.target(USER, User { id: 42 }).unwrap();
    let matched = routes.resolve(&target).unwrap();
    assert_eq!(*matched.params(USER).unwrap(), User { id: 42 });
    assert!(matched.chain[0].params.0.is_empty());
    assert!(matches!(
        routes.resolve(&Location::parse("/users/no").unwrap()),
        Err(Error::BadParams(_))
    ));
    assert_eq!(
        routes
            .resolve(&Location::parse("/").unwrap())
            .unwrap()
            .chain
            .last()
            .unwrap()
            .id,
        HOME.id()
    );
}
#[test]
fn failed_static_candidate_falls_back_without_leaking_parameters() {
    let narrow = RouteKey::<Params>::new("narrow");
    let broad = RouteKey::<Params>::new("broad");
    let routes = Routes::builder()
        .root(RouteDef::new(narrow, "a/:wrong/x"))
        .root(RouteDef::new(broad, ":prefix/:value/y"))
        .finish()
        .unwrap();
    let matched = routes.resolve(&Location::parse("/a/v/y").unwrap()).unwrap();
    let params = matched.params(broad).unwrap();
    assert!(!params.0.contains_key("wrong"));
    assert_eq!(params.required::<String>("prefix").unwrap(), "a");
}
#[test]
fn encoded_slashes_catch_all_and_query_round_trip_on_all_hosts() {
    let location = Location::parse("/files/a%2Fb/%252F?q=a+b&q=%2B#%E4%B8%AD").unwrap();
    assert_eq!(location.segments().unwrap(), ["files", "a/b", "%2F"]);
    assert_eq!(
        location.query().repeated::<String>("q").unwrap(),
        ["a b", "+"]
    );
    assert!(location.query().optional::<String>("q").is_err());
    assert_eq!(location.fragment(), Some("中"));
    assert_eq!(Location::parse(&location.to_string()).unwrap(), location);
    let files = RouteKey::<Params>::new("files");
    let routes = Routes::builder()
        .root(RouteDef::new(files, "files/*rest"))
        .finish()
        .unwrap();
    let captured = routes.resolve(&location).unwrap().params(files).unwrap();
    assert_eq!(captured.rest("rest").unwrap(), ["a/b", "%2F"]);
    let rebuilt = routes.target(files, (*captured).clone()).unwrap();
    assert_eq!(rebuilt.segments().unwrap(), location.segments().unwrap());
}

#[test]
fn relative_targets_use_consumed_route_paths_and_cannot_escape_the_mount() {
    let routes = Routes::builder()
        .base("/app")
        .root(RouteDef::layout(ROOT, "").child(RouteDef::new(USER, "users/:id")))
        .finish()
        .unwrap();
    let current = Location::parse("/app/users/42?tab=a").unwrap();
    assert_eq!(
        routes
            .resolve_relative(USER.id(), &current, "edit")
            .unwrap()
            .path(),
        "/app/users/42/edit"
    );
    assert_eq!(
        routes
            .resolve_relative(ROOT.id(), &current, "help")
            .unwrap()
            .path(),
        "/app/help"
    );
    assert_eq!(
        routes
            .resolve_relative(USER.id(), &current, "#details")
            .unwrap()
            .to_string(),
        "/app/users/42?tab=a#details"
    );
    assert_eq!(
        routes
            .resolve_relative(USER.id(), &current, "?tab=b")
            .unwrap()
            .to_string(),
        "/app/users/42?tab=b"
    );
    assert_eq!(
        routes.resolve_relative(USER.id(), &current, "../../../../escape"),
        Err(Error::OutsideBase)
    );
    assert!(
        routes
            .resolve_relative(USER.id(), &current, "https://external.test/")
            .is_err()
    );
}
#[test]
fn optional_index_and_empty_catch_all_have_deterministic_priority() {
    let optional = RouteKey::<Params>::new("optional");
    let without_index = Routes::builder()
        .root(RouteDef::new(REST, "*rest"))
        .root(RouteDef::new(optional, ":id?"))
        .finish()
        .unwrap();
    assert_eq!(
        without_index
            .resolve(&Location::parse("/").unwrap())
            .unwrap()
            .chain[0]
            .id,
        optional.id()
    );
    let routes = Routes::builder()
        .root(RouteDef::new(REST, "*rest"))
        .root(RouteDef::new(optional, ":id?"))
        .root(RouteDef::new(HOME, ""))
        .finish()
        .unwrap();
    assert_eq!(
        routes
            .resolve(&Location::parse("/").unwrap())
            .unwrap()
            .chain[0]
            .id,
        HOME.id()
    );
    assert_eq!(
        routes
            .resolve(&Location::parse("/one").unwrap())
            .unwrap()
            .chain[0]
            .id,
        optional.id()
    );
    assert_eq!(
        routes
            .resolve(&Location::parse("/one/two").unwrap())
            .unwrap()
            .chain[0]
            .id,
        REST.id()
    );
}
#[test]
fn invalid_definitions_base_boundaries_slash_and_limits() {
    assert!(
        Routes::builder()
            .root(RouteDef::new(USER, ":id"))
            .root(RouteDef::new(REST, ":rest"))
            .finish()
            .is_err()
    );
    assert!(
        Routes::builder()
            .root(RouteDef::new(REST, "*rest/tail"))
            .finish()
            .is_err()
    );
    let routes = Routes::builder()
        .base("/app")
        .root(RouteDef::new(HOME, ""))
        .finish()
        .unwrap();
    assert!(routes.resolve(&Location::parse("/apple").unwrap()).is_err());
    assert!(routes.resolve(&Location::parse("/APP").unwrap()).is_err());
    assert!(routes.resolve(&Location::parse("/app").unwrap()).is_ok());
    let strict = Routes::builder()
        .root(RouteDef::new(HOME, "about/"))
        .finish()
        .unwrap();
    assert!(strict.resolve(&Location::parse("/about").unwrap()).is_err());
    assert!(strict.resolve(&Location::parse("/about/").unwrap()).is_ok());
    let canonical = Routes::builder()
        .slash_policy(SlashPolicy::RedirectWithoutSlash)
        .root(RouteDef::new(HOME, "about"))
        .finish()
        .unwrap();
    assert_eq!(
        canonical
            .canonical(&Location::parse("/about/?x=1#h").unwrap())
            .to_string(),
        "/about?x=1#h"
    );
    for bad in ["//external.test/a", "/bad%", "/bad%FF", "/a\\b", "/a\nb"] {
        assert!(Location::parse(bad).is_err(), "{bad}");
    }
    let limits = Limits {
        url_bytes: 4,
        ..Limits::default()
    };
    assert!(Location::parse_with("/large", limits).is_err());
    let limits = Limits {
        match_steps: 1,
        ..Limits::default()
    };
    let bounded = Routes::builder()
        .limits(limits)
        .root(RouteDef::new(HOME, "about"))
        .finish()
        .unwrap();
    assert!(matches!(
        bounded.resolve(&Location::parse("/about").unwrap()),
        Err(Error::OverLimit(_))
    ));
    assert!(Location::parse("/a#").unwrap().fragment().is_some());
    assert!(Location::parse("/a").unwrap().fragment().is_none());
}
