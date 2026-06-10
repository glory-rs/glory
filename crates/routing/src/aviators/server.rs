use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::holder::Enabler;
use glory_core::Truck;

use crate::{Aviator, Handler, Locator, NavigationError, Router};

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct ServerAviator {
    pub truck: Rc<RefCell<Truck>>,
    pub router: Rc<Router>,
    #[educe(Debug(ignore))]
    pub catcher: Rc<dyn Handler>,
    base_path: String,
}

impl ServerAviator {
    pub fn new(router: impl Into<Rc<Router>>, catcher: impl Handler) -> Self {
        Self {
            truck: Default::default(),
            router: router.into(),
            catcher: Rc::new(catcher),
            base_path: Default::default(),
        }
    }
    pub(crate) fn locate(&self, raw_url: impl Into<String>) -> Result<(), NavigationError> {
        super::run_route(&self.truck, &self.router, &self.catcher, raw_url.into())
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
