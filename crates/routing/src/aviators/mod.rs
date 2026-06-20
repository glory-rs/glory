mod scroll;
pub use scroll::{DEFAULT_SCROLL_MEMORY_CAPACITY, ScrollMemory, ScrollPosition};

cfg_feature! {
    #![all(target_arch = "wasm32", feature = "web-csr")]
    mod browser;
    pub use browser::BrowserAviator;
}

cfg_feature! {
    #![feature ="web-ssr"]
    mod server;
    pub use server::ServerAviator;
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod memory;
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use memory::MemoryAviator;

/// Shared route-matching pass used by the non-browser aviators: parse the
/// URL, run matched hoops + goal (or the catcher), then publish the result
/// through the [`Locator`](crate::Locator) in the truck.
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub(crate) fn run_route(
    truck: &std::rc::Rc<std::cell::RefCell<glory_core::Truck>>,
    router: &std::rc::Rc<crate::Router>,
    catcher: &std::rc::Rc<dyn crate::Handler>,
    raw_url: String,
) -> Result<(), crate::NavigationError> {
    use crate::url::Url;
    use crate::{Locator, NavigationError, PathState};

    let url = Url::parse(&raw_url)?;
    glory_core::info!("[locate]: {:?}", url);
    let locator = truck
        .borrow_mut()
        .scrape::<Locator>()
        .map_err(|_| NavigationError::NotEnabled("locator not found"))?;
    let mut detect_state = PathState::new(url.path());
    let matched = router.detect(&url, &truck.borrow(), &mut detect_state);
    if let Some(dm) = matched {
        for hoop in [&dm.hoops[..], &[dm.goal]].concat() {
            hoop.handle(truck.clone());
        }
    } else {
        glory_core::info!("No matched route found for {:?}", raw_url);
        catcher.handle(truck.clone());
    }
    truck.borrow_mut().inject(locator.clone());
    locator.receive(raw_url, Some(detect_state.params))?;
    Ok(())
}
