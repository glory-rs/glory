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
    assert_eq!(
        AppRoute::Files {
            path: vec!["docs".to_owned(), "intro page.md".to_owned()],
        }
        .to_url(),
        "/files/docs/intro%20page%2Emd"
    );
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
