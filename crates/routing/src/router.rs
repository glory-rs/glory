use std::cell::RefCell;
use std::fmt::{self, Formatter};
use std::rc::Rc;

#[cfg(all(not(feature = "__single_holder"), feature = "salvo"))]
use glory_core::web::holders::SalvoHandler;
#[cfg(all(not(feature = "__single_holder"), feature = "salvo"))]
use indexmap::IndexSet;

use super::{Filter, FnFilter, PathFilter, PathState};
use crate::url::Url;
use crate::{DetectMatched, Handler, Truck, WhenHoop};

#[macro_export]
macro_rules! join_path {
    ($($part:expr),+) => {
        {
            let mut p = std::path::PathBuf::new();
            $(
                p.push($part);
            )*
            path_slash::PathBufExt::to_slash_lossy(&p).to_string()
        }
    }
}

/// Router struct is used for router request to different handlers.
///
/// This form of definition can make the definition of router clear and simple for complex projects.
#[non_exhaustive]
pub struct Router {
    /// routers is the children of current router.
    pub routers: Vec<Router>,
    /// filters is the filters of current router.
    pub filters: Vec<Box<dyn Filter>>,
    /// hoops of current router.
    pub hoops: Vec<Rc<dyn Handler>>,
    /// goal of current router.
    pub goal: Option<Rc<dyn Handler>>,
}

impl Default for Router {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Create a new `Router`.
    #[inline]
    pub fn new() -> Self {
        Self {
            routers: Vec::new(),
            filters: Vec::new(),
            hoops: Vec::new(),
            goal: None,
        }
    }

    /// Get current router's children reference.
    #[inline]
    pub fn routers(&self) -> &Vec<Router> {
        &self.routers
    }
    /// Get current router's children mutable reference.
    #[inline]
    pub fn routes_mut(&mut self) -> &mut Vec<Router> {
        &mut self.routers
    }

    /// Get current router's middlewares reference.
    #[inline]
    pub fn hoops(&self) -> &Vec<Rc<dyn Handler>> {
        &self.hoops
    }
    /// Get current router's middlewares mutable reference.
    #[inline]
    pub fn hoops_mut(&mut self) -> &mut Vec<Rc<dyn Handler>> {
        &mut self.hoops
    }

    /// Get current router's filters reference.
    #[inline]
    pub fn filters(&self) -> &Vec<Box<dyn Filter>> {
        &self.filters
    }
    /// Get current router's filters mutable reference.
    #[inline]
    pub fn filters_mut(&mut self) -> &mut Vec<Box<dyn Filter>> {
        &mut self.filters
    }

    /// Detect current router is matched for current request.
    pub fn detect(&self, url: &Url, truck: &Truck, path_state: &mut PathState) -> Option<DetectMatched> {
        for filter in &self.filters {
            if !filter.filter(url, truck, path_state) {
                return None;
            }
        }
        if !self.routers.is_empty() {
            let original_cursor = path_state.cursor;
            for child in &self.routers {
                if let Some(dm) = child.detect(url, truck, path_state) {
                    return Some(DetectMatched {
                        hoops: [&self.hoops[..], &dm.hoops[..]].concat(),
                        goal: dm.goal.clone(),
                    });
                } else {
                    path_state.cursor = original_cursor;
                }
            }
        }
        if let Some(goal) = self.goal.clone() {
            if path_state.is_ended() {
                return Some(DetectMatched {
                    hoops: self.hoops.clone(),
                    goal,
                });
            }
        }
        None
    }

    /// Push a router as child of current router.
    #[inline]
    pub fn push(mut self, router: Router) -> Self {
        self.routers.push(router);
        self
    }
    /// Append all routers in a Vec as children of current router.
    #[inline]
    pub fn append(mut self, others: &mut Vec<Router>) -> Self {
        self.routers.append(others);
        self
    }

    /// Add a handler as middleware, it will run the handler in current router or it's descendants
    /// handle the request.
    #[inline]
    pub fn with_hoop<H: Handler>(handler: H) -> Self {
        Router::new().hoop(handler)
    }

    /// Add a handler as middleware, it will run the handler in current router or it's descendants
    /// handle the request. This middleware only effective when the filter return true.
    #[inline]
    pub fn with_hoop_when<H, F>(hoop: H, filter: F) -> Self
    where
        H: Handler,
        F: Fn(&Truck) -> bool + 'static,
    {
        Router::new().hoop_when(hoop, filter)
    }

    /// Add a handler as middleware, it will run the handler in current router or it's descendants
    /// handle the request.
    #[inline]
    pub fn hoop<H: Handler>(mut self, hoop: H) -> Self {
        self.hoops.push(Rc::new(hoop));
        self
    }

    /// Add a handler as middleware, it will run the handler in current router or it's descendants
    /// handle the request. This middleware only effective when the filter return true.
    #[inline]
    pub fn hoop_when<H, F>(mut self, hoop: H, filter: F) -> Self
    where
        H: Handler,
        F: Fn(&Truck) -> bool + 'static,
    {
        self.hoops.push(Rc::new(WhenHoop { inner: hoop, filter }));
        self
    }

    /// Create a new router and set path filter.
    ///
    /// # Panics
    ///
    /// Panics if path value is not in correct format.
    #[inline]
    pub fn with_path(path: impl Into<String>) -> Self {
        Router::with_filter(PathFilter::new(path))
    }

    /// Create a new path filter for current router.
    ///
    /// # Panics
    ///
    /// Panics if path value is not in correct format.
    #[inline]
    pub fn path(self, path: impl Into<String>) -> Self {
        self.filter(PathFilter::new(path))
    }

    /// Create a new router and set filter.
    #[inline]
    pub fn with_filter(filter: impl Filter + Sized) -> Self {
        Router::new().filter(filter)
    }
    /// Add a filter for current router.
    #[inline]
    pub fn filter(mut self, filter: impl Filter + Sized) -> Self {
        self.filters.push(Box::new(filter));
        self
    }

    /// Create a new router and set filter_fn.
    #[inline]
    pub fn with_filter_fn<T>(func: T) -> Self
    where
        T: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        Router::with_filter(FnFilter(func))
    }
    /// Create a new FnFilter from Fn.
    #[inline]
    pub fn filter_fn<T>(self, func: T) -> Self
    where
        T: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        self.filter(FnFilter(func))
    }

    /// Sets current router's goal handler.
    #[inline]
    pub fn goal<H: Handler>(mut self, goal: H) -> Self {
        self.goal = Some(Rc::new(goal));
        self
    }

    /// When you want write router chain, this function will be useful,
    /// You can write your custom logic in FnOnce.
    #[inline]
    pub fn then<F>(self, func: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        func(self)
    }
    cfg_feature! {
        #![all(not(feature = "__single_holder"),feature = "salvo")]
        pub fn make_salvo_router(&self, handler: SalvoHandler) -> salvo::Router {
            let mut all_paths = IndexSet::new();
            let root_path = self.filtered_path();
            if let Some(root_path) = &root_path {
                if self.goal.is_some() {
                    all_paths.insert(root_path.to_owned());
                }
            }

            fn add_child(all_paths: &mut IndexSet<String>, parent_path: &str, router: &Router) {
                let cur_path = router.filtered_path().map(|s| join_path!(parent_path, s)).unwrap_or(parent_path.to_owned());
                    if router.goal.is_some() {
                        all_paths.insert(cur_path.clone());
                    }
                    for router in &router.routers {
                        add_child(all_paths, &cur_path, router);
                    }
            }
            for router in &self.routers {
                add_child(&mut all_paths, root_path.as_deref().unwrap_or_default(), router);
            }

            let mut all_paths  = all_paths.into_iter().rev().collect::<Vec<_>>();

            let mut root = salvo::Router::new();
            fn add_path(all_paths: &mut Vec<String>, parent: &mut salvo::Router, path: String, handler: SalvoHandler) {
                let path  = if !path.ends_with('/') {
                    format!("{path}/")
                }   else {
                    path.to_owned()
                };
                let mut child_paths = Vec::new();
                for child_path in all_paths.iter() {
                    if child_path.starts_with(&path) {
                        child_paths.push(child_path.clone());
                    }
                }
                all_paths.retain(|p| !child_paths.contains(&p));
                if !path.is_empty() && path != "/" {
                    let mut router = salvo::Router::with_path(path.clone()).get(handler.clone());
                    for child_path in child_paths {
                        add_path(all_paths, &mut router, child_path, handler.clone());
                    }
                    parent.routers.push(router);
                } else {
                    for child_path in child_paths {
                        add_path(all_paths, parent, child_path, handler.clone());
                    }
                }
            }

            while !all_paths.is_empty() {
                let path = all_paths.pop().unwrap();
                add_path(&mut all_paths, &mut root, path, handler.clone());
            }
            root
        }
    }

    #[allow(unused)]
    fn filtered_path(&self) -> Option<String> {
        for filter in &self.filters {
            let info = format!("{filter:?}");
            if info.starts_with("path:") {
                let path = info.split_once(':').unwrap().1.to_owned();
                return Some(path);
            }
        }
        None
    }
}

const SYMBOL_DOWN: &str = "│";
const SYMBOL_TEE: &str = "├";
const SYMBOL_ELL: &str = "└";
const SYMBOL_RIGHT: &str = "─";
impl fmt::Debug for Router {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fn print(f: &mut Formatter, prefix: &str, last: bool, router: &Router) -> fmt::Result {
            let mut path = "".to_owned();
            let mut others = Vec::with_capacity(router.filters.len());
            if router.filters.is_empty() {
                path = "!NULL!".to_owned();
            } else {
                for filter in &router.filters {
                    let info = format!("{filter:?}");
                    if info.starts_with("path:") {
                        path = info.split_once(':').unwrap().1.to_owned();
                    } else {
                        let mut parts = info.splitn(2, ':').collect::<Vec<_>>();
                        if !parts.is_empty() {
                            others.push(parts.pop().unwrap().to_owned());
                        }
                    }
                }
            }
            let cp = if last {
                format!("{prefix}{SYMBOL_ELL}{SYMBOL_RIGHT}{SYMBOL_RIGHT}")
            } else {
                format!("{prefix}{SYMBOL_TEE}{SYMBOL_RIGHT}{SYMBOL_RIGHT}")
            };
            let hd = if let Some(goal) = &router.goal {
                format!(" -> {}", goal.type_name())
            } else {
                "".into()
            };
            if !others.is_empty() {
                writeln!(f, "{cp}{path}[{}]{hd}", others.join(","))?;
            } else {
                writeln!(f, "{cp}{path}{hd}")?;
            }
            let routers = router.routers();
            if !routers.is_empty() {
                let np = if last {
                    format!("{prefix}    ")
                } else {
                    format!("{prefix}{SYMBOL_DOWN}   ")
                };
                for (i, router) in routers.iter().enumerate() {
                    print(f, &np, i == routers.len() - 1, router)?;
                }
            }
            Ok(())
        }
        print(f, "", true, self)
    }
}
