use crate::{Entry, Error, History, Location, MatchPlan, Result, Routes, Traversal};
use glory_core::{Bond, Cage, Owner, Scope};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationKind {
    Push,
    Replace,
    Traverse(i32),
    Observed,
    Revalidate,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Navigation {
    Committed,
    Noop,
    Boundary,
    Accepted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Redirect(Location),
    Block,
    Denied(u16),
}
#[derive(Clone)]
pub struct GuardContext {
    pub location: Location,
    pub kind: NavigationKind,
    pub router: RouterHandle,
}
pub type GuardFuture = Pin<Box<dyn Future<Output = Result<Decision>>>>;
type Guard = Rc<dyn Fn(GuardContext) -> Result<Decision>>;
type AsyncGuard = Rc<dyn Fn(GuardContext) -> GuardFuture>;

#[derive(Clone)]
pub struct Snapshot {
    pub entry: Entry,
    pub plan: MatchPlan,
    pub error: Option<Error>,
    pub revision: u64,
    pub kind: NavigationKind,
}
impl Snapshot {
    pub fn location(&self) -> &Location {
        &self.entry.location
    }
}
struct Flight {
    identity: Rc<()>,
    waker: RefCell<Option<Waker>>,
    owner: RefCell<Option<Rc<Scope>>>,
}
struct Control {
    scope: Scope,
    routes: Routes,
    history: Rc<dyn History>,
    state: Cage<Rc<Snapshot>>,
    guards: RefCell<Vec<Guard>>,
    async_guards: RefCell<Vec<AsyncGuard>>,
    pending: RefCell<Option<Rc<Flight>>>,
    busy: Cell<bool>,
}
/// The parent Owner retains control state. Dropping a temporary Router does
/// not retire a mounted application; disposing its Owner invalidates handles.
pub struct Router(Rc<Control>);
#[derive(Clone)]
pub struct RouterHandle(Weak<Control>);

fn plan(routes: &Routes, location: &Location) -> Result<(MatchPlan, Option<Error>)> {
    match routes.resolve(location) {
        Ok(plan) => Ok((plan, None)),
        Err(Error::NoMatch) => Ok((MatchPlan::default(), Some(Error::NoMatch))),
        Err(error) => Err(error),
    }
}
impl Router {
    pub fn new(parent: &Scope, routes: Routes, history: Rc<dyn History>) -> Result<Self> {
        if !parent.is_alive() {
            return Err(Error::Gone);
        }
        parent.owner().check_write()?;
        let entry = history.current()?;
        let canonical = routes.canonical(&entry.location);
        let (plan, error) = plan(&routes, &canonical)?;
        let entry = if canonical != entry.location {
            history.replace(&canonical)?
        } else {
            entry
        };
        let scope = parent.child();
        let state = scope.cage(Rc::new(Snapshot {
            entry,
            plan,
            error,
            revision: 0,
            kind: NavigationKind::Revalidate,
        }));
        let control = Rc::new(Control {
            scope,
            routes,
            history,
            state,
            guards: RefCell::new(Vec::new()),
            async_guards: RefCell::new(Vec::new()),
            pending: RefCell::new(None),
            busy: Cell::new(false),
        });
        let retained = control.clone();
        parent.on_cleanup(move || {
            retained.cancel();
            retained.scope.owner().dispose();
        })?;
        Ok(Self(control))
    }
    pub fn handle(&self) -> RouterHandle {
        RouterHandle(Rc::downgrade(&self.0))
    }
    pub fn guard(&self, guard: impl Fn(GuardContext) -> Result<Decision> + 'static) {
        self.0.guards.borrow_mut().push(Rc::new(guard));
    }
    pub fn async_guard(&self, guard: impl Fn(GuardContext) -> GuardFuture + 'static) {
        self.0.async_guards.borrow_mut().push(Rc::new(guard));
    }
}
impl Control {
    fn cancel(&self) {
        let pending = self.pending.borrow_mut().take();
        if let Some(pending) = pending {
            if let Some(owner) = pending.owner.borrow_mut().take() {
                owner.owner().dispose();
            }
            if let Some(waker) = pending.waker.borrow_mut().take() {
                waker.wake();
            }
        }
    }
    fn start(&self) -> Rc<Flight> {
        self.cancel();
        let flight = Rc::new(Flight {
            identity: Rc::new(()),
            waker: RefCell::new(None),
            owner: RefCell::new(None),
        });
        *self.pending.borrow_mut() = Some(flight.clone());
        flight
    }
    fn current(&self) -> Result<Rc<Snapshot>> {
        self.state.try_with_untracked(Rc::clone).map_err(Into::into)
    }
    fn publish(
        &self,
        entry: Entry,
        plan: MatchPlan,
        error: Option<Error>,
        kind: NavigationKind,
    ) -> Result {
        let revision = self
            .current()?
            .revision
            .checked_add(1)
            .ok_or(Error::OverLimit("navigation revision"))?;
        self.state.try_revise(|state| {
            *state = Rc::new(Snapshot {
                entry,
                plan,
                error,
                revision,
                kind,
            })
        })?;
        Ok(())
    }
    fn is_current(&self, flight: &Flight) -> bool {
        self.scope.is_alive()
            && self
                .pending
                .borrow()
                .as_ref()
                .is_some_and(|pending| Rc::ptr_eq(&pending.identity, &flight.identity))
    }
}
struct Busy<'a>(&'a Cell<bool>);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl RouterHandle {
    fn control(&self) -> Result<Rc<Control>> {
        self.0
            .upgrade()
            .filter(|control| control.scope.is_alive())
            .ok_or(Error::Gone)
    }
    fn writable_control(&self) -> Result<Rc<Control>> {
        let control = self.control()?;
        control.scope.owner().check_write()?;
        Ok(control)
    }
    pub fn is_alive(&self) -> bool {
        self.control().is_ok()
    }
    pub fn cancel_pending(&self) -> Result {
        self.writable_control()?.cancel();
        Ok(())
    }
    /// Tracked inside reactive computations; use current() for untracked reads.
    pub fn snapshot(&self) -> Result<Rc<Snapshot>> {
        Ok(self.control()?.state.try_get()?)
    }
    pub fn current(&self) -> Result<Rc<Snapshot>> {
        self.control()?.current()
    }
    /// Tracked path-only active-link matching, with real segment boundaries.
    pub fn is_active_path(&self, target: &Location, descendants: bool) -> Result<bool> {
        let snapshot = self.snapshot()?;
        let current = snapshot.location().segments()?;
        let expected = target.segments()?;
        if descendants {
            Ok(current.starts_with(&expected))
        } else {
            Ok(current == expected
                && snapshot.location().path().ends_with('/') == target.path().ends_with('/'))
        }
    }
    pub fn routes(&self) -> Result<Routes> {
        Ok(self.control()?.routes.clone())
    }
    pub fn runtime(&self) -> Result<glory_core::RuntimeId> {
        Ok(self.control()?.scope.runtime())
    }
    /// None means the host cannot know the traversal boundary in advance.
    pub fn can_traverse(&self, delta: i32) -> Result<Option<bool>> {
        Ok(match self.control()?.history.preview(delta)? {
            Traversal::Known(_) => Some(true),
            Traversal::Boundary => Some(false),
            Traversal::Host => None,
        })
    }
    pub fn has_async_guards(&self) -> Result<bool> {
        Ok(!self.control()?.async_guards.borrow().is_empty())
    }
    pub fn location(&self, owner: &Owner) -> Result<Bond<Location>> {
        if !owner.is_alive() {
            return Err(Error::Gone);
        }
        let control = self.control()?;
        if owner.runtime() != control.scope.runtime() {
            return Err(Error::WrongRuntime);
        }
        let state = control.state;
        Ok(owner.bond(move || state.get().entry.location.clone()))
    }
    pub fn push(&self, location: Location) -> Result<Navigation> {
        self.prepare(location, NavigationKind::Push)?.commit()
    }
    pub fn push_entry(&self, location: Location) -> Result<Navigation> {
        let mut prepared = self.prepare(location, NavigationKind::Push)?;
        prepared.force = true;
        prepared.commit()
    }
    pub fn replace(&self, location: Location) -> Result<Navigation> {
        self.prepare(location, NavigationKind::Replace)?.commit()
    }
    pub fn revalidate(&self) -> Result<Navigation> {
        let location = self.current()?.entry.location.clone();
        self.prepare(location, NavigationKind::Revalidate)?.commit()
    }
    pub fn back(&self) -> Result<Navigation> {
        self.traverse(-1)
    }
    pub fn forward(&self) -> Result<Navigation> {
        self.traverse(1)
    }
    pub fn traverse(&self, delta: i32) -> Result<Navigation> {
        let control = self.writable_control()?;
        match control.history.preview(delta)? {
            Traversal::Boundary => Ok(Navigation::Boundary),
            Traversal::Known(entry) => self
                .prepare(entry.location, NavigationKind::Traverse(delta))?
                .commit(),
            Traversal::Host => {
                control.cancel();
                control.history.traverse(delta)?;
                Ok(Navigation::Accepted)
            }
        }
    }
    /// An interactive native back gesture prepares without changing the UI or
    /// cursor. Drop cancels; commit fails if another navigation intervened.
    pub fn prepare_back(&self) -> Result<Option<PreparedNavigation>> {
        match self.control()?.history.preview(-1)? {
            Traversal::Boundary => Ok(None),
            Traversal::Known(entry) => self
                .prepare(entry.location, NavigationKind::Traverse(-1))
                .map(Some),
            Traversal::Host => Err(Error::Unsupported("interactive traversal preview")),
        }
    }
    pub fn prepare(&self, location: Location, kind: NavigationKind) -> Result<PreparedNavigation> {
        let control = self.writable_control()?;
        if !control.async_guards.borrow().is_empty() {
            return Err(Error::AsyncRequired);
        }
        if control.busy.replace(true) {
            return Err(Error::Busy);
        }
        let _busy = Busy(&control.busy);
        let flight = control.start();
        let (location, plan, error) = self.resolve_sync(&control, location, kind)?;
        let expected = if let NavigationKind::Traverse(delta) = kind {
            match control.history.preview(delta)? {
                Traversal::Known(entry) => Some(entry),
                _ => return Err(Error::StaleNavigation),
            }
        } else {
            None
        };
        Ok(PreparedNavigation {
            router: self.clone(),
            location,
            plan,
            error,
            kind,
            revision: control.current()?.revision,
            flight,
            force: false,
            expected,
        })
    }
    fn resolve_sync(
        &self,
        control: &Control,
        mut location: Location,
        kind: NavigationKind,
    ) -> Result<(Location, MatchPlan, Option<Error>)> {
        let guards = control.guards.borrow().clone();
        let mut visited = Vec::new();
        for _ in 0..16 {
            location = control.routes.canonical(&Location::parse_with(
                &location.to_string(),
                control.routes.limits(),
            )?);
            if visited.contains(&location) {
                return Err(Error::RedirectLoop);
            }
            visited.push(location.clone());
            let (matched, error) = plan(&control.routes, &location)?;
            let mut redirect = None;
            for guard in &guards {
                let context = GuardContext {
                    location: location.clone(),
                    kind,
                    router: self.clone(),
                };
                match guard(context)? {
                    Decision::Allow => {}
                    Decision::Redirect(next) => {
                        redirect = Some(next);
                        break;
                    }
                    Decision::Block => return Err(Error::Blocked),
                    Decision::Denied(status) => return Err(Error::Denied(status)),
                }
            }
            if let Some(next) = redirect {
                location = next;
            } else {
                return Ok((location, matched, error));
            }
        }
        Err(Error::RedirectLoop)
    }
    /// Call from the host's queued event/frame boundary, not a native listener.
    /// The external address is already committed; refusal becomes an error view.
    pub fn observe(&self, entry: Entry) -> Result<Navigation> {
        let control = self.writable_control()?;
        if control.busy.replace(true) {
            return Err(Error::Busy);
        }
        let _busy = Busy(&control.busy);
        control.cancel();
        if control.current()?.entry == entry {
            return Ok(Navigation::Noop);
        }
        control.history.observe(&entry)?;
        let resolved = if control.async_guards.borrow().is_empty() {
            self.resolve_sync(&control, entry.location.clone(), NavigationKind::Observed)
        } else {
            Err(Error::AsyncRequired)
        };
        match resolved {
            Ok((location, plan, error)) => {
                let entry = if entry.location != location {
                    control.history.replace(&location)?
                } else {
                    entry
                };
                control.publish(entry, plan, error, NavigationKind::Observed)?;
            }
            Err(error) => {
                control.publish(
                    entry,
                    MatchPlan::default(),
                    Some(error),
                    NavigationKind::Observed,
                )?;
            }
        }
        Ok(Navigation::Committed)
    }
    /// Poll on an explicitly supplied local executor. New navigation wakes and
    /// cancels the old guard future; this method never installs a hidden executor.
    pub async fn push_async(&self, location: Location) -> Result<Navigation> {
        self.prepare_async(location, NavigationKind::Push)
            .await?
            .commit()
    }
    pub async fn replace_async(&self, location: Location) -> Result<Navigation> {
        self.prepare_async(location, NavigationKind::Replace)
            .await?
            .commit()
    }
    pub async fn observe_async(&self, entry: Entry) -> Result<Navigation> {
        let control = self.writable_control()?;
        control.cancel();
        if control.current()?.entry == entry {
            return Ok(Navigation::Noop);
        }
        control.history.observe(&entry)?;
        match self
            .prepare_async(entry.location.clone(), NavigationKind::Observed)
            .await
        {
            Ok(prepared) => prepared.commit(),
            Err(Error::Superseded) => Err(Error::Superseded),
            Err(error) => {
                if control.history.current()? != entry {
                    return Err(Error::Superseded);
                }
                control.publish(
                    entry,
                    MatchPlan::default(),
                    Some(error),
                    NavigationKind::Observed,
                )?;
                Ok(Navigation::Committed)
            }
        }
    }
    pub async fn prepare_async(
        &self,
        location: Location,
        kind: NavigationKind,
    ) -> Result<PreparedNavigation> {
        let control = self.writable_control()?;
        if control.busy.get() {
            return Err(Error::Busy);
        }
        let expected = match kind {
            NavigationKind::Traverse(delta) => match control.history.preview(delta)? {
                Traversal::Known(entry) => Some(entry),
                _ => return Err(Error::Unsupported("async traversal preview")),
            },
            NavigationKind::Observed => Some(control.history.current()?),
            _ => None,
        };
        let flight = control.start();
        let pending_owner = Rc::new(control.scope.child());
        *flight.owner.borrow_mut() = Some(pending_owner.clone());
        let _lease = GuardOwner {
            scope: pending_owner.clone(),
            flight: flight.clone(),
        };
        let guards = control.async_guards.borrow().clone();
        let mut next = location;
        let mut visited = Vec::new();
        for _ in 0..16 {
            let (location, matched, error) = {
                if control.busy.replace(true) {
                    return Err(Error::Busy);
                }
                let _busy = Busy(&control.busy);
                self.resolve_sync(&control, next, kind)?
            };
            if visited.contains(&location) {
                return Err(Error::RedirectLoop);
            }
            visited.push(location.clone());
            let mut redirect = None;
            for guard in &guards {
                let context = GuardContext {
                    location: location.clone(),
                    kind,
                    router: self.clone(),
                };
                let future = pending_owner.owner().enter(|| guard(context));
                let decision = Cancellable {
                    router: self.clone(),
                    flight: flight.clone(),
                    future,
                    owner: pending_owner.owner(),
                }
                .await?;
                match decision {
                    Decision::Allow => {}
                    Decision::Redirect(next) => {
                        redirect = Some(next);
                        break;
                    }
                    Decision::Block => return Err(Error::Blocked),
                    Decision::Denied(status) => return Err(Error::Denied(status)),
                }
            }
            if !control.is_current(&flight) {
                return Err(Error::Superseded);
            }
            if let Some(location) = redirect {
                next = location;
                continue;
            }
            return Ok(PreparedNavigation {
                router: self.clone(),
                location,
                plan: matched,
                error,
                kind,
                revision: control.current()?.revision,
                flight,
                force: false,
                expected,
            });
        }
        Err(Error::RedirectLoop)
    }
}

pub struct PreparedNavigation {
    router: RouterHandle,
    location: Location,
    plan: MatchPlan,
    error: Option<Error>,
    kind: NavigationKind,
    revision: u64,
    flight: Rc<Flight>,
    force: bool,
    expected: Option<Entry>,
}
impl PreparedNavigation {
    pub fn location(&self) -> &Location {
        &self.location
    }
    pub fn commit(self) -> Result<Navigation> {
        let control = self.router.writable_control()?;
        if !control.is_current(&self.flight) || control.current()?.revision != self.revision {
            return Err(Error::StaleNavigation);
        }
        if control.busy.replace(true) {
            return Err(Error::Busy);
        }
        let _busy = Busy(&control.busy);
        let current = control.current()?;
        if self.kind != NavigationKind::Observed && control.history.current()? != current.entry {
            return Err(Error::StaleNavigation);
        }
        if self.kind == NavigationKind::Push
            && !self.force
            && current.entry.location == self.location
        {
            return Ok(Navigation::Noop);
        }
        let entry = match self.kind {
            NavigationKind::Push => control.history.push(&self.location)?,
            NavigationKind::Replace => control.history.replace(&self.location)?,
            NavigationKind::Traverse(delta) => {
                if !matches!(control.history.preview(delta)?,Traversal::Known(ref entry) if Some(entry) == self.expected.as_ref())
                {
                    return Err(Error::StaleNavigation);
                }
                let entry = control
                    .history
                    .traverse(delta)?
                    .ok_or(Error::StaleNavigation)?;
                if entry.location != self.location {
                    control.history.replace(&self.location)?
                } else {
                    entry
                }
            }
            NavigationKind::Revalidate => {
                if current.entry.location == self.location {
                    current.entry.clone()
                } else {
                    control.history.replace(&self.location)?
                }
            }
            NavigationKind::Observed => {
                let entry = self.expected.ok_or(Error::StaleNavigation)?;
                if control.history.current()? != entry {
                    return Err(Error::StaleNavigation);
                }
                if entry.location != self.location {
                    control.history.replace(&self.location)?
                } else {
                    entry
                }
            }
        };
        control.publish(entry, self.plan, self.error, self.kind)?;
        Ok(Navigation::Committed)
    }
}
struct GuardOwner {
    scope: Rc<Scope>,
    flight: Rc<Flight>,
}
impl Drop for GuardOwner {
    fn drop(&mut self) {
        self.scope.owner().dispose();
        self.flight.owner.borrow_mut().take();
    }
}
struct Cancellable<'a> {
    router: RouterHandle,
    flight: Rc<Flight>,
    future: GuardFuture,
    owner: &'a Owner,
}
impl Future for Cancellable<'_> {
    type Output = Result<Decision>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let control = match self.router.control() {
            Ok(control) => control,
            Err(error) => return Poll::Ready(Err(error)),
        };
        if !control.is_current(&self.flight) {
            return Poll::Ready(Err(Error::Superseded));
        }
        *self.flight.waker.borrow_mut() = Some(cx.waker().clone());
        let owner = self.owner;
        owner.enter(|| self.future.as_mut().poll(cx))
    }
}
