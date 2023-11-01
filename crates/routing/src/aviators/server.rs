use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::holder::Enabler;
use glory_core::Truck;

use crate::url::Url;
use crate::{Aviator, Handler, Locator, PathState, Router};

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
    pub(crate) fn locate(&self, raw_url: impl Into<String>) -> Result<(), url::ParseError> {
        let raw_url = raw_url.into();
        let url = Url::parse(&raw_url)?;
        glory_core::info!("[locate]: {:?}", url);
        let locator = self.truck.borrow_mut().scrape::<Locator>().expect("locator not found");
        let mut detect_state = PathState::new(url.path());
        let matched = self.router.detect(&url, &self.truck.borrow(), &mut detect_state);
        if let Some(dm) = matched {
            for hoop in [&dm.hoops[..], &[dm.goal]].concat() {
                hoop.handle(self.truck.clone());
            }
        } else {
            glory_core::info!("No matched route found for {:?}", raw_url);
            self.catcher.handle(self.truck.clone());
        }
        self.truck.borrow_mut().inject(locator.clone());
        locator.receive(raw_url, detect_state.params)
    }
}

impl Aviator for ServerAviator {
    fn goto(&self, _url: &str) {
        panic!("ServerAviator::goto() is not implemented");
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
        self.locate(url).unwrap();
    }
}
