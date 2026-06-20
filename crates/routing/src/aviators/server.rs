use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::holder::Enabler;
use glory_core::Truck;

use crate::{Aviator, Handler, Locator, NavigationError, Routable, Router};

/// Rewrites an incoming URL before route matching, applying declarative
/// redirects such as those declared by `#[redirect]` on a [`Routable`] type.
///
/// Returns the rewritten URL, or `None` to leave the URL untouched.
type RedirectResolver = Rc<dyn Fn(&str) -> Option<String>>;

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct ServerAviator {
    pub truck: Rc<RefCell<Truck>>,
    pub router: Rc<Router>,
    #[educe(Debug(ignore))]
    pub catcher: Rc<dyn Handler>,
    base_path: String,
    #[educe(Debug(ignore))]
    redirect_resolver: Option<RedirectResolver>,
}

impl ServerAviator {
    pub fn new(router: impl Into<Rc<Router>>, catcher: impl Handler) -> Self {
        Self {
            truck: Default::default(),
            router: router.into(),
            catcher: Rc::new(catcher),
            base_path: Default::default(),
            redirect_resolver: None,
        }
    }

    /// Install a [`Routable`] type as the server-side declarative redirect
    /// resolver.
    ///
    /// Before each `locate`, the incoming URL is resolved with
    /// [`Routable::resolve_url`] (which applies `#[redirect]` rules first and a
    /// `#[not_found]` fallback last). When that yields a route whose
    /// [`Routable::to_url`] differs from the requested URL, navigation proceeds
    /// against the rewritten target. This makes routes that configure
    /// `#[redirect]` behave the same during SSR as they do in the browser.
    pub fn with_redirects<R>(mut self) -> Self
    where
        R: Routable,
    {
        self.redirect_resolver = Some(Rc::new(|url: &str| R::resolve_url(url).map(|route| route.to_url())));
        self
    }

    /// Install a custom redirect resolver that rewrites the incoming URL before
    /// route matching. Returning `None` keeps the original URL.
    pub fn with_redirect_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&str) -> Option<String> + 'static,
    {
        self.redirect_resolver = Some(Rc::new(resolver));
        self
    }

    /// Apply the configured redirect resolver (if any) to `raw_url`, returning
    /// the URL that route matching should run against.
    fn resolve_redirect(&self, raw_url: String) -> String {
        match &self.redirect_resolver {
            Some(resolver) => match resolver(&raw_url) {
                Some(target) if target != raw_url => {
                    glory_core::info!("[redirect]: {raw_url} -> {target}");
                    target
                }
                _ => raw_url,
            },
            None => raw_url,
        }
    }

    pub(crate) fn locate(&self, raw_url: impl Into<String>) -> Result<(), NavigationError> {
        let raw_url = self.resolve_redirect(raw_url.into());
        super::run_route(&self.truck, &self.router, &self.catcher, raw_url)
    }
}

impl Aviator for ServerAviator {
    fn goto(&self, url: &str) -> Result<(), NavigationError> {
        self.locate(url)
    }
}

impl Enabler for ServerAviator {
    fn enable(mut self, truck: Rc<RefCell<Truck>>) {
        let url = {
            let truck = truck.borrow();
            truck.get::<String>("glory::url").unwrap().clone()
        };
        truck.borrow_mut().inject(Locator::new());
        self.truck = truck.clone();
        if let Err(err) = self.locate(url) {
            glory_core::warn!("ServerAviator failed to locate initial URL: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Routable, TruckExt};
    use glory_core::{Scope, Widget};

    #[derive(Debug)]
    struct UserPage;
    impl Widget for UserPage {
        fn build(&mut self, _ctx: &mut Scope) {}
    }

    #[derive(Debug)]
    struct NotFoundPage;
    impl Widget for NotFoundPage {
        fn build(&mut self, _ctx: &mut Scope) {}
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum AppRoute {
        User { id: u64 },
        NotFound { raw_url: String },
    }

    impl Routable for AppRoute {
        fn to_url(&self) -> String {
            match self {
                Self::User { id } => format!("/users/{id}"),
                Self::NotFound { .. } => "/404".to_owned(),
            }
        }

        fn from_url(url: &str) -> Option<Self> {
            let matched = crate::match_route_pattern(url, "/users/<id>")?;
            Some(Self::User { id: matched.param("id").ok()? })
        }

        fn redirect(url: &str) -> Option<Self> {
            crate::redirect_url(url, "/u/<id>", |m| Some(Self::User { id: m.param("id").ok()? }))
        }

        fn not_found(url: &str) -> Option<Self> {
            Some(Self::NotFound { raw_url: url.to_owned() })
        }
    }

    fn build_aviator() -> ServerAviator {
        let router = Rc::new(Router::new().push(Router::with_path("users/<id>").goal(|truck: Rc<RefCell<Truck>>| truck.insert_stuff("page@user", UserPage))));
        let catcher: Rc<dyn Handler> = Rc::new(|truck: Rc<RefCell<Truck>>| truck.insert_stuff("page@catcher", NotFoundPage));
        let truck = Rc::new(RefCell::new(Truck::default()));
        truck.borrow_mut().inject(Locator::new());
        let mut aviator = ServerAviator::new(router, move |truck: Rc<RefCell<Truck>>| catcher.handle(truck));
        aviator.truck = truck;
        aviator
    }

    #[test]
    fn ssr_redirect_resolver_rewrites_legacy_url_to_target() {
        let aviator = build_aviator().with_redirects::<AppRoute>();
        aviator.locate("/u/42".to_owned()).unwrap();

        let locator = aviator.truck.borrow_mut().scrape::<Locator>().unwrap();
        // The legacy `/u/42` URL must have been rewritten to the redirect target.
        assert_eq!(&*locator.path().get_untracked(), "/users/42");
        assert!(aviator.truck.contains_stuff_key("page", "user"), "redirect target route should match the user goal");
    }

    #[test]
    fn ssr_redirect_resolver_leaves_normal_url_untouched() {
        let aviator = build_aviator().with_redirects::<AppRoute>();
        aviator.locate("/users/7".to_owned()).unwrap();

        let locator = aviator.truck.borrow_mut().scrape::<Locator>().unwrap();
        assert_eq!(&*locator.path().get_untracked(), "/users/7");
        assert!(aviator.truck.contains_stuff_key("page", "user"));
    }

    #[test]
    fn ssr_without_resolver_keeps_legacy_url() {
        let aviator = build_aviator();
        aviator.locate("/u/42".to_owned()).unwrap();

        let locator = aviator.truck.borrow_mut().scrape::<Locator>().unwrap();
        // No resolver installed: the legacy URL is left as-is and falls to the catcher.
        assert_eq!(&*locator.path().get_untracked(), "/u/42");
        assert!(aviator.truck.contains_stuff_key("page", "catcher"));
    }
}
