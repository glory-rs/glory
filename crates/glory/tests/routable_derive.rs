use glory::routing::Routable as _;

#[derive(Debug, Clone, PartialEq, Eq, glory::Routable)]
enum AppRoute {
    #[route("/")]
    Home,
    #[route("/users/<id>")]
    #[redirect("/u/<id>")]
    User { id: u64 },
    #[route("/files/<**path>")]
    Files { path: Vec<String> },
    #[route("/posts/<id>/<slug>")]
    Post(u32, String),
    #[route("/strict/<id:num>")]
    Strict { id: u32 },
    #[not_found]
    NotFound { raw_url: String },
}

#[test]
fn derive_routable_generates_urls() {
    assert_eq!(AppRoute::Home.to_url(), "/");
    assert_eq!(AppRoute::User { id: 42 }.to_url(), "/users/42");
    // Unreserved characters (`-`, `.`, `_`, `~`) stay literal; only what is
    // actually reserved in a path segment gets encoded. This has to hold on
    // wasm too — the browser and the server must generate the same href, or
    // hydration sees a different attribute than the SSR markup.
    assert_eq!(
        AppRoute::Files {
            path: vec!["docs".to_owned(), "intro page.md".to_owned()],
        }
        .to_url(),
        "/files/docs/intro%20page.md"
    );
    // A separator inside a parameter must not be able to open a new segment,
    // a query or a fragment.
    assert_eq!(AppRoute::Post(7, "a/b?c=1#d".to_owned()).to_url(), "/posts/7/a%2Fb%3Fc%3D1%23d");
    assert_eq!(AppRoute::Post(7, "hello world".to_owned()).to_url(), "/posts/7/hello%20world");
    assert_eq!(AppRoute::Strict { id: 9 }.to_url(), "/strict/9");
    assert_eq!(
        AppRoute::NotFound {
            raw_url: "/missing".to_owned(),
        }
        .to_url(),
        "/missing"
    );
}

#[test]
fn derive_routable_parses_urls() {
    assert_eq!(AppRoute::from_url("/"), Some(AppRoute::Home));
    assert_eq!(AppRoute::from_url("/users/42"), Some(AppRoute::User { id: 42 }));
    assert_eq!(
        AppRoute::from_url("/files/docs/intro%20page.md"),
        Some(AppRoute::Files {
            path: vec!["docs".to_owned(), "intro page.md".to_owned()],
        })
    );
    assert_eq!(
        AppRoute::from_url("/posts/7/hello%20world"),
        Some(AppRoute::Post(7, "hello world".to_owned()))
    );
    assert_eq!(AppRoute::from_url("/strict/9"), Some(AppRoute::Strict { id: 9 }));
    assert_eq!(AppRoute::from_url("/strict/not-a-number"), None);
    assert_eq!(AppRoute::from_url("/users/not-a-number"), None);
}

#[test]
fn derive_routable_resolves_redirects_and_not_found() {
    assert_eq!(AppRoute::resolve_url("/u/42"), Some(AppRoute::User { id: 42 }));
    assert_eq!(
        AppRoute::resolve_url("/missing"),
        Some(AppRoute::NotFound {
            raw_url: "/missing".to_owned(),
        })
    );
}

#[test]
fn derived_catch_all_decodes_exactly_once() {
    // Regression: the router decoded each path segment, and the derived
    // `from_url` decoded the captured tail a second time. `%252e%252e` came
    // back as `..`, so a path check written against the parsed segments could
    // be walked straight past.
    let Some(AppRoute::Files { path }) = AppRoute::from_url("/files/%252e%252e/secret") else {
        panic!("catch-all route must match");
    };
    assert_eq!(path, vec!["%2e%2e".to_owned(), "secret".to_owned()], "exactly one decode pass");

    // An ordinary encoded segment still decodes once.
    let Some(AppRoute::Files { path }) = AppRoute::from_url("/files/docs/intro%20page.md") else {
        panic!("catch-all route must match");
    };
    assert_eq!(path, vec!["docs".to_owned(), "intro page.md".to_owned()]);
}

#[test]
fn derived_single_param_decodes_exactly_once() {
    let Some(AppRoute::Post(id, slug)) = AppRoute::from_url("/posts/7/a%252Fb") else {
        panic!("post route must match");
    };
    assert_eq!(id, 7);
    assert_eq!(slug, "a%2Fb");
}

/// A later variant must stay reachable when an earlier one matches the shape
/// of the URL but fails to parse its parameters.
#[derive(Debug, Clone, PartialEq, Eq, glory::Routable)]
enum OrderedRoute {
    // Declared first and matches `/users/<anything>`, but only parses digits.
    #[route("/users/<id>")]
    User { id: u64 },
    #[route("/users/new")]
    New,
    #[not_found]
    NotFound { raw_url: String },
}

#[test]
fn later_variants_are_reachable_when_an_earlier_one_fails_to_parse() {
    assert_eq!(OrderedRoute::from_url("/users/42"), Some(OrderedRoute::User { id: 42 }));

    // Regression: the derived `from_url` used `?` on the parameter parse, which
    // returned from the whole function. `/users/new` matched `/users/<id>`
    // first, `u64` parsing failed, and the route fell all the way through to
    // `#[not_found]` — so declaration order silently decided whether a route
    // was reachable at all.
    assert_eq!(OrderedRoute::from_url("/users/new"), Some(OrderedRoute::New));

    // A URL matching neither still resolves to the fallback.
    assert_eq!(
        OrderedRoute::resolve_url("/users/nope"),
        Some(OrderedRoute::NotFound {
            raw_url: "/users/nope".to_owned()
        })
    );
}
