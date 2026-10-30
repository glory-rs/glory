//! OS-facing navigation input, usable with any renderer and on Android/iOS.
//! Deliver queued events at the application's observation boundary.
use crate::{Location, Navigation, PreparedNavigation, Result, RouterHandle};

pub enum NativeEvent {
    Back,
    Forward,
    Open(Location),
}
pub struct NativeNavigation {
    router: RouterHandle,
    gesture: Option<PreparedNavigation>,
}
impl NativeNavigation {
    pub fn new(router: RouterHandle) -> Self {
        Self {
            router,
            gesture: None,
        }
    }
    /// Boundary means the OS/application should handle back (e.g. dismiss/exit).
    pub fn dispatch(&mut self, event: NativeEvent) -> Result<Navigation> {
        self.cancel_back();
        match event {
            NativeEvent::Back => self.router.back(),
            NativeEvent::Forward => self.router.forward(),
            NativeEvent::Open(location) => self.router.push(location),
        }
    }
    pub fn begin_back(&mut self) -> Result<bool> {
        self.cancel_back();
        self.gesture = self.router.prepare_back()?;
        Ok(self.gesture.is_some())
    }
    pub fn cancel_back(&mut self) {
        self.gesture.take();
    }
    pub fn finish_back(&mut self) -> Result<Navigation> {
        match self.gesture.take() {
            Some(gesture) => gesture.commit(),
            None => Ok(Navigation::Noop),
        }
    }
    /// Convert an OS-delivered URL using an explicit application scheme/host
    /// mapping. This does not register schemes or open any external service.
    pub fn deep_link(&mut self, value: &str, scheme: &str, host: &str) -> Result<Navigation> {
        let url =
            url::Url::parse(value).map_err(|error| crate::Error::BadUrl(error.to_string()))?;
        if url.scheme() != scheme
            || url.host_str() != Some(host)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return Err(crate::Error::Unsupported(
                "deep link outside configured application",
            ));
        }
        let query = url
            .query()
            .map(|query| format!("?{query}"))
            .unwrap_or_default();
        let fragment = url
            .fragment()
            .map(|fragment| format!("#{fragment}"))
            .unwrap_or_default();
        self.dispatch(NativeEvent::Open(Location::parse(&format!(
            "{}{query}{fragment}",
            url.path()
        ))?))
    }
}
