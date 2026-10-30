use crate::{Error, Params, Result, RouteId, RouteKey, RouteParams, RouterHandle, Routes};
use glory_core::{Bond, Owner, Scope};
use glory_ui::{Widget, each};
use std::{
    any::Any,
    cell::Cell,
    collections::BTreeMap,
    marker::PhantomData,
    rc::{Rc, Weak},
};

type Factory = Rc<dyn Fn(RouteContext<Params>) -> Box<dyn Widget>>;
type InstanceKey = Rc<dyn Fn(&Rc<dyn Any>) -> Result<String>>;
#[derive(Default)]
struct Views {
    factories: BTreeMap<RouteId, Factory>,
    keys: BTreeMap<RouteId, InstanceKey>,
    fallback: Option<Rc<dyn Fn(RouterHandle) -> Box<dyn Widget>>>,
}
/// Local UI factories are kept separate from shareable Routes metadata.
pub struct RouteViews {
    routes: Routes,
    views: Views,
}
impl RouteViews {
    pub fn new(routes: Routes) -> Self {
        Self {
            routes,
            views: Views::default(),
        }
    }
    pub fn route<P: RouteParams, W: Widget>(
        mut self,
        key: RouteKey<P>,
        factory: impl Fn(RouteContext<P>) -> W + 'static,
    ) -> Result<Self> {
        if !self.routes.contains(key) {
            return Err(Error::UnknownRoute(key.id.0.into()));
        }
        if self.views.factories.contains_key(&key.id) {
            return Err(Error::Definition("duplicate view".into()));
        }
        self.views
            .factories
            .insert(key.id, Rc::new(move |ctx| Box::new(factory(ctx.cast()))));
        Ok(self)
    }
    pub fn key_by<P: RouteParams>(
        mut self,
        key: RouteKey<P>,
        key_fn: impl Fn(&P) -> String + 'static,
    ) -> Result<Self> {
        if !self.routes.contains(key) {
            return Err(Error::UnknownRoute(key.id.0.into()));
        }
        self.views.keys.insert(
            key.id,
            Rc::new(move |value| {
                value
                    .downcast_ref::<P>()
                    .map(&key_fn)
                    .ok_or(Error::WrongParamType)
            }),
        );
        Ok(self)
    }
    pub fn fallback<W: Widget>(mut self, factory: impl Fn(RouterHandle) -> W + 'static) -> Self {
        self.views.fallback = Some(Rc::new(move |router| Box::new(factory(router))));
        self
    }
    pub fn view(self, router: RouterHandle) -> Result<RouteView> {
        if !self.routes.same_table(&router.routes()?) {
            return Err(Error::Definition(
                "Router and RouteViews must share Routes".into(),
            ));
        }
        let snapshot = router.current()?;
        for matched in &snapshot.plan.chain {
            if !self.views.factories.contains_key(&matched.id) {
                return Err(Error::Definition(format!("missing view: {}", matched.id.0)));
            }
        }
        Ok(RouteView {
            router,
            routes: self.routes,
            views: Rc::new(self.views),
            depth: 0,
            mounted: Rc::new(Cell::new(false)),
        })
    }
}
pub struct RouteContext<P> {
    router: RouterHandle,
    routes: Routes,
    views: Weak<Views>,
    depth: usize,
    id: RouteId,
    instance: glory_core::InstanceId,
    outlet_mounted: Rc<Cell<bool>>,
    marker: PhantomData<fn() -> P>,
}
impl<P> Clone for RouteContext<P> {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            routes: self.routes.clone(),
            views: self.views.clone(),
            depth: self.depth,
            id: self.id,
            instance: self.instance,
            outlet_mounted: self.outlet_mounted.clone(),
            marker: PhantomData,
        }
    }
}
impl<P> RouteContext<P> {
    fn cast<T>(self) -> RouteContext<T> {
        RouteContext {
            router: self.router,
            routes: self.routes,
            views: self.views,
            depth: self.depth,
            id: self.id,
            instance: self.instance,
            outlet_mounted: self.outlet_mounted,
            marker: PhantomData,
        }
    }
    pub fn router(&self) -> RouterHandle {
        self.router.clone()
    }
    pub fn resolve(&self, relative: &str) -> Result<crate::Location> {
        if !self.instance.is_alive() {
            return Err(Error::Gone);
        }
        self.routes
            .resolve_relative(self.id, self.router.snapshot()?.location(), relative)
    }
    pub fn outlet(&self) -> Outlet {
        Outlet {
            router: self.router.clone(),
            routes: self.routes.clone(),
            views: self.views.clone(),
            depth: self.depth + 1,
            mounted: self.outlet_mounted.clone(),
        }
    }
}
impl<P: RouteParams> RouteContext<P> {
    pub fn read_params(&self) -> Result<Rc<P>> {
        if !self.instance.is_alive() {
            return Err(Error::Gone);
        }
        self.router
            .snapshot()?
            .plan
            .chain
            .get(self.depth)
            .ok_or(Error::Gone)?
            .typed(RouteKey::<P>::new(self.id.0))
    }
    pub fn params(&self, owner: &Owner) -> Result<Bond<Result<Rc<P>>>> {
        if !owner.is_alive() {
            return Err(Error::Gone);
        }
        self.read_params()?;
        if owner.runtime() != self.instance.runtime() {
            return Err(Error::WrongRuntime);
        }
        let context = self.clone();
        Ok(owner.bond(move || context.read_params()))
    }
}
pub struct RouteView {
    router: RouterHandle,
    routes: Routes,
    views: Rc<Views>,
    depth: usize,
    mounted: Rc<Cell<bool>>,
}
pub struct Outlet {
    router: RouterHandle,
    routes: Routes,
    views: Weak<Views>,
    depth: usize,
    mounted: Rc<Cell<bool>>,
}
impl Widget for Outlet {
    fn compose(&mut self, scope: &Scope) -> glory_ui::Result<Option<Box<dyn Widget>>> {
        let mut view = RouteView {
            router: self.router.clone(),
            routes: self.routes.clone(),
            views: self.views.upgrade().ok_or(glory_ui::Error::Gone)?,
            depth: self.depth,
            mounted: self.mounted.clone(),
        };
        view.compose(scope)
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
struct Branch {
    ids: Vec<(RouteId, String)>,
    fallback: Option<String>,
}
fn branch(snapshot: &crate::Snapshot, views: &Views, depth: usize) -> Result<Option<Branch>> {
    if let Some(error) = &snapshot.error {
        return Ok((depth == 0).then(|| Branch {
            ids: Vec::new(),
            fallback: Some(error.to_string()),
        }));
    }
    if snapshot.plan.chain.get(depth).is_none() {
        return Ok(None);
    }
    let ids = snapshot.plan.chain[..=depth]
        .iter()
        .map(|matched| {
            let key = views
                .keys
                .get(&matched.id)
                .map(|key| key(&matched.value))
                .transpose()?
                .unwrap_or_default();
            Ok((matched.id, key))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(Branch {
        ids,
        fallback: None,
    }))
}
fn ui_error(error: Error) -> glory_ui::Error {
    glory_render::Error::Backend(error.to_string()).into()
}
impl Widget for RouteView {
    fn compose(&mut self, scope: &Scope) -> glory_ui::Result<Option<Box<dyn Widget>>> {
        if self.mounted.replace(true) {
            return Err(glory_ui::Error::Gone);
        }
        let router = self.router.clone();
        let views = self.views.clone();
        let depth = self.depth;
        let initial = router.current().map_err(ui_error)?;
        branch(&initial, &views, depth).map_err(ui_error)?;
        let source = scope.bond(move || {
            router
                .snapshot()
                .ok()
                .and_then(|snapshot| branch(&snapshot, &views, depth).ok().flatten())
                .into_iter()
                .collect::<Vec<_>>()
        });
        let router = self.router.clone();
        let views = self.views.clone();
        let routes = self.routes.clone();
        let content =
            each(source)
                .key(|branch| branch.clone())
                .seed(move |slot| -> Box<dyn Widget> {
                    if slot.fallback.is_some() {
                        return views
                            .fallback
                            .as_ref()
                            .map(|factory| factory(router.clone()))
                            .unwrap_or_else(|| {
                                Box::new(
                                    glory_ui::text().content(slot.fallback.unwrap_or_default()),
                                )
                            });
                    }
                    let id = slot.ids.last().expect("non-empty route chain").0;
                    Box::new(Page {
                        router: router.clone(),
                        routes: routes.clone(),
                        views: views.clone(),
                        depth,
                        id,
                    })
                });
        Ok(Some(Box::new(content)))
    }
}
struct Page {
    router: RouterHandle,
    routes: Routes,
    views: Rc<Views>,
    depth: usize,
    id: RouteId,
}
impl Widget for Page {
    fn compose(&mut self, scope: &Scope) -> glory_ui::Result<Option<Box<dyn Widget>>> {
        let context = RouteContext::<Params> {
            router: self.router.clone(),
            routes: self.routes.clone(),
            views: Rc::downgrade(&self.views),
            depth: self.depth,
            id: self.id,
            instance: scope.id(),
            outlet_mounted: Rc::new(Cell::new(false)),
            marker: PhantomData,
        };
        let factory =
            self.views.factories.get(&self.id).ok_or_else(|| {
                ui_error(Error::Definition(format!("missing view: {}", self.id.0)))
            })?;
        Ok(Some(Box::new(CheckedPage {
            factory: factory.clone(),
            context,
        })))
    }
}
struct CheckedPage {
    factory: Factory,
    context: RouteContext<Params>,
}
impl Widget for CheckedPage {
    fn build(&mut self, scope: &glory_ui::Scope) -> glory_ui::Result {
        let mut widget = (self.factory)(self.context.clone());
        widget.build(scope)?;
        if self
            .context
            .router
            .current()
            .map_err(ui_error)?
            .plan
            .chain
            .len()
            > self.context.depth + 1
            && !self.context.outlet_mounted.get()
        {
            return Err(ui_error(Error::Definition(format!(
                "missing outlet: {}",
                self.context.id.0
            ))));
        }
        scope.retain(widget)
    }
}
