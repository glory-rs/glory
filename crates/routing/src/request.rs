//! Request resolution returns HTTP-neutral outcomes; transport adapters decide
//! how to send headers and render the matched/fallback Widget.
use crate::{Decision, Entry, Error, History, Location, MatchPlan, Result, Routes, Traversal};

pub struct RequestLocation(Location);
pub struct RequestResolution {
    pub location: Location,
    pub status: u16,
    pub redirect: Option<Location>,
    pub error: Option<Error>,
}
impl RequestLocation {
    pub fn parse(target: &str) -> Result<Self> {
        if target.contains('#') {
            return Err(Error::BadUrl(
                "HTTP request target cannot contain a fragment".into(),
            ));
        }
        Location::parse(target).map(Self)
    }
    pub fn resolve(&self, routes: &Routes) -> RequestResolution {
        self.resolve_with(routes, |_, _| Ok(Decision::Allow))
    }
    /// Request services are explicit closure inputs. Called before HTML/shell
    /// emission; redirects produce a real outcome, not an internal rewrite.
    pub fn resolve_with(
        &self,
        routes: &Routes,
        mut guard: impl FnMut(&Location, &MatchPlan) -> Result<Decision>,
    ) -> RequestResolution {
        let mut location = self.0.clone();
        let mut visited = Vec::new();
        for _ in 0..16 {
            location = routes.canonical(&location);
            if visited.contains(&location) {
                return request_error(location, Error::RedirectLoop);
            }
            visited.push(location.clone());
            let (plan, error) = match routes.resolve(&location) {
                Ok(plan) => (plan, None),
                Err(Error::NoMatch) => (MatchPlan::default(), Some(Error::NoMatch)),
                Err(error) => return request_error(location, error),
            };
            match guard(&location, &plan) {
                Ok(Decision::Redirect(next)) => {
                    location = next;
                    continue;
                }
                Ok(Decision::Allow) => {
                    let redirect = (location != self.0).then(|| location.clone());
                    let status = if redirect.is_some() {
                        if visited.len() == 1 { 308 } else { 302 }
                    } else if error.is_some() {
                        404
                    } else {
                        200
                    };
                    return RequestResolution {
                        location,
                        status,
                        redirect,
                        error,
                    };
                }
                Ok(Decision::Denied(status)) if (400..600).contains(&status) => {
                    return RequestResolution {
                        location,
                        status,
                        redirect: None,
                        error: Some(Error::Denied(status)),
                    };
                }
                Ok(Decision::Denied(_)) | Ok(Decision::Block) => {
                    return request_error(location, Error::Denied(403));
                }
                Err(error) => return request_error(location, error),
            }
        }
        request_error(location, Error::RedirectLoop)
    }
}
fn request_error(location: Location, error: Error) -> RequestResolution {
    let status = match error {
        Error::BadUrl(_) | Error::BadParams(_) => 400,
        Error::OverLimit(_) => 414,
        Error::Denied(status) => status,
        Error::NoMatch | Error::OutsideBase => 404,
        _ => 500,
    };
    RequestResolution {
        location,
        status,
        redirect: None,
        error: Some(error),
    }
}
/// Immutable per-request history; no browser, OS or global state is consulted.
pub struct RequestHistory(Entry);
impl RequestHistory {
    pub fn new(location: Location) -> Self {
        Self(Entry {
            location,
            key: None,
        })
    }
}
impl History for RequestHistory {
    fn current(&self) -> Result<Entry> {
        Ok(self.0.clone())
    }
    fn push(&self, _: &Location) -> Result<Entry> {
        Err(Error::Unsupported("navigation during request rendering"))
    }
    fn replace(&self, _: &Location) -> Result<Entry> {
        Err(Error::Unsupported(
            "resolve redirects before request rendering",
        ))
    }
    fn preview(&self, _: i32) -> Result<Traversal> {
        Ok(Traversal::Boundary)
    }
    fn traverse(&self, _: i32) -> Result<Option<Entry>> {
        Err(Error::Unsupported("request traversal"))
    }
}
