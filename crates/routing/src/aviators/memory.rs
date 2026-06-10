use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::Truck;
use glory_core::holder::Enabler;

use crate::{Aviator, Handler, Locator, NavigationError, Router};

/// In-memory history backend for hosts without a browser history API:
/// desktop / mobile windows, tests, and future native shells.
///
/// Navigation semantics mirror the browser: [`Aviator::goto`] truncates any
/// forward entries and pushes the new URL; [`MemoryAviator::back`] /
/// [`MemoryAviator::forward`] move the cursor and re-run route matching for
/// the target entry.
#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct MemoryAviator {
    pub truck: Rc<RefCell<Truck>>,
    pub router: Rc<Router>,
    #[educe(Debug(ignore))]
    pub catcher: Rc<dyn Handler>,
    entries: Rc<RefCell<Vec<String>>>,
    cursor: Rc<RefCell<usize>>,
    initial_url: String,
}

impl MemoryAviator {
    pub fn new(router: impl Into<Rc<Router>>, catcher: impl Handler) -> Self {
        Self {
            truck: Default::default(),
            router: router.into(),
            catcher: Rc::new(catcher),
            entries: Rc::new(RefCell::new(Vec::new())),
            cursor: Rc::new(RefCell::new(0)),
            initial_url: "/".to_owned(),
        }
    }

    /// URL routed when the aviator is enabled (defaults to `/`). A
    /// `glory::url` Truck entry, when present, takes precedence.
    pub fn initial_url(mut self, url: impl Into<String>) -> Self {
        self.initial_url = url.into();
        self
    }

    /// The URL the cursor currently points at.
    pub fn current(&self) -> Option<String> {
        self.entries.borrow().get(*self.cursor.borrow()).cloned()
    }

    /// Re-routes to the previous history entry. Returns `false` when
    /// already at the oldest entry.
    pub fn back(&self) -> Result<bool, NavigationError> {
        let target = {
            let mut cursor = self.cursor.borrow_mut();
            if *cursor == 0 {
                return Ok(false);
            }
            *cursor -= 1;
            self.entries.borrow()[*cursor].clone()
        };
        super::run_route(&self.truck, &self.router, &self.catcher, target)?;
        Ok(true)
    }

    /// Re-routes to the next history entry. Returns `false` when already
    /// at the newest entry.
    pub fn forward(&self) -> Result<bool, NavigationError> {
        let target = {
            let mut cursor = self.cursor.borrow_mut();
            if *cursor + 1 >= self.entries.borrow().len() {
                return Ok(false);
            }
            *cursor += 1;
            self.entries.borrow()[*cursor].clone()
        };
        super::run_route(&self.truck, &self.router, &self.catcher, target)?;
        Ok(true)
    }

    fn push(&self, url: String) {
        let mut entries = self.entries.borrow_mut();
        let mut cursor = self.cursor.borrow_mut();
        if !entries.is_empty() {
            entries.truncate(*cursor + 1);
        }
        entries.push(url);
        *cursor = entries.len() - 1;
    }
}

impl Aviator for MemoryAviator {
    fn goto(&self, url: &str) -> Result<(), NavigationError> {
        super::run_route(&self.truck, &self.router, &self.catcher, url.to_owned())?;
        self.push(url.to_owned());
        Ok(())
    }
}

impl Enabler for MemoryAviator {
    fn enable(mut self, truck: Rc<RefCell<Truck>>) {
        let url = match truck.borrow().get::<String>("glory::url") {
            Ok(url) => url.clone(),
            Err(_) => self.initial_url.clone(),
        };
        truck.borrow_mut().inject(Locator::new());
        self.truck = truck.clone();
        match super::run_route(&self.truck, &self.router, &self.catcher, url.clone()) {
            Ok(()) => self.push(url),
            Err(err) => glory_core::warn!("MemoryAviator failed to locate initial URL: {err}"),
        }
    }
}
