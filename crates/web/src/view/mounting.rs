//! Backend adapters for one configuration/structural-control implementation.
use super::*;
type UiResult<T = ()> = glory_ui::Result<T>;

/// Internal destination for named configuration actions, not a component API.
#[doc(hidden)]
pub struct Mount<'a> {
    backend: Backend<'a>,
}
enum Backend<'a> {
    Ui(&'a portable::Mount<'a>),
    #[cfg(target_arch = "wasm32")]
    Sparse {
        definition: &'a Compiled,
        app: &'a crate::template::App,
        scope: &'a Scope,
        instance: &'a crate::template::Instance,
    },
}
impl<'a> Mount<'a> {
    pub(crate) fn ui(mount: &'a portable::Mount<'a>) -> Self {
        Self {
            backend: Backend::Ui(mount),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn sparse(
        definition: &'a Compiled,
        app: &'a crate::template::App,
        scope: &'a Scope,
        instance: &'a crate::template::Instance,
    ) -> Self {
        Self {
            backend: Backend::Sparse {
                definition,
                app,
                scope,
                instance,
            },
        }
    }
    fn slot(&self, name: &str) -> UiResult<u16> {
        match &self.backend {
            Backend::Ui(m) => Ok(m.frame.slot(name)?.0),
            #[cfg(target_arch = "wasm32")]
            Backend::Sparse { definition, .. } => definition
                .slot(name)
                .map(|(slot, _)| slot)
                .ok_or(glory_ui::Error::Gone),
        }
    }
    pub(crate) fn text<V: TextValue + 'static>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.slot(field.0)?;
        match &self.backend {
            Backend::Ui(m) => {
                let frame = m.frame.clone();
                if value.reactive_text() {
                    let weak = m.scope.downgrade();
                    m.scope.binding(move || {
                        let result = value
                            .read_text()
                            .map_err(portable::failure)
                            .and_then(|text| frame.text(slot, text).map_err(Into::into));
                        if let Err(error) = result
                            && let Some(scope) = weak.upgrade()
                        {
                            scope.report_error(error);
                        }
                    });
                    Ok(())
                } else {
                    frame.text(
                        slot,
                        glory_core::untracked(|| value.read_text()).map_err(portable::failure)?,
                    )?;
                    Ok(())
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Sparse {
                scope, instance, ..
            } => {
                if value.reactive_text() {
                    instance
                        .try_bind_values(scope, [slot], move || Ok([value.read_text()?]))
                        .map_err(portable::failure)
                } else {
                    instance
                        .set(
                            slot,
                            glory_core::untracked(|| value.read_text())
                                .map_err(portable::failure)?,
                        )
                        .map_err(portable::failure)
                }
            }
        }
    }
    pub(crate) fn flag<V: BoolValue>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.slot(field.0)?;
        match &self.backend {
            Backend::Ui(m) => {
                let frame = m.frame.clone();
                if value.reactive() {
                    let weak = m.scope.downgrade();
                    m.scope.binding(move || {
                        let result = value
                            .read_bool()
                            .map_err(portable::failure)
                            .and_then(|flag| frame.flag(slot, flag).map_err(Into::into));
                        if let Err(error) = result
                            && let Some(scope) = weak.upgrade()
                        {
                            scope.report_error(error);
                        }
                    });
                    Ok(())
                } else {
                    frame.flag(
                        slot,
                        glory_core::untracked(|| value.read_bool()).map_err(portable::failure)?,
                    )?;
                    Ok(())
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Sparse {
                scope, instance, ..
            } => {
                if value.reactive() {
                    instance
                        .bind_flag(scope, slot, move || value.read_bool())
                        .map_err(portable::failure)
                } else {
                    let next = value.read_bool().map_err(portable::failure)?;
                    if next != instance.flag_initial(slot).map_err(portable::failure)? {
                        instance.set_flag(slot, next).map_err(portable::failure)?;
                    }
                    Ok(())
                }
            }
        }
    }
    pub(crate) fn on(&self, click: Click, callback: impl FnMut() + 'static) -> UiResult {
        let slot = self.slot(click.0)?;
        match &self.backend {
            Backend::Ui(m) => {
                m.handlers.borrow_mut().insert(slot, Box::new(callback));
                Ok(())
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Sparse { instance, .. } => {
                instance.on_click(slot, callback).map_err(portable::failure)
            }
        }
    }
    pub(crate) fn child<C: Content>(&self, outlet: Outlet, content: C) -> UiResult {
        match &self.backend {
            Backend::Ui(m) => {
                let scope = m.outlet(outlet)?;
                content.install(&scope, &ContentMount::ui(&scope))
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Sparse {
                app,
                scope,
                instance,
                ..
            } => {
                let parent = instance
                    .children(self.slot(outlet.0)?)
                    .map_err(portable::failure)?;
                content.install(scope, &ContentMount::sparse(app, scope, parent, instance))
            }
        }
    }
}

/// Weak parent/host references shared by Each, Show, Content and lazy factories.
#[doc(hidden)]
#[derive(Clone)]
pub struct ContentMount {
    backend: Parent,
}
#[derive(Clone)]
enum Parent {
    Ui(glory_ui::WeakScope),
    #[cfg(target_arch = "wasm32")]
    Sparse {
        app: crate::template::WeakApp,
        owner: glory_core::InstanceId,
        parent: crate::template::Children,
        instance: crate::template::Instance,
    },
}
pub(crate) enum Child {
    Ui(glory_ui::Mounted),
    #[cfg(target_arch = "wasm32")]
    Sparse {
        view: super::Mounted,
        scope: Scope,
    },
}
impl Child {
    #[cfg(target_arch = "wasm32")]
    fn ui(&self) -> Option<&glory_ui::Mounted> {
        match self {
            Self::Ui(child) => Some(child),
            #[cfg(target_arch = "wasm32")]
            _ => None,
        }
    }
}
impl ContentMount {
    pub(crate) fn ui(scope: &glory_ui::Scope) -> Self {
        Self {
            backend: Parent::Ui(scope.downgrade()),
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn sparse(
        app: &crate::template::App,
        scope: &Scope,
        parent: crate::template::Children,
        instance: &crate::template::Instance,
    ) -> Self {
        Self {
            backend: Parent::Sparse {
                app: app.downgrade(),
                owner: scope.id(),
                parent,
                instance: instance.clone(),
            },
        }
    }
    pub(crate) fn mount<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult<Child> {
        match &self.backend {
            Parent::Ui(parent) => Ok(Child::Ui(
                parent
                    .upgrade()
                    .ok_or(glory_ui::Error::Gone)?
                    .mount_with(factory)?,
            )),
            #[cfg(target_arch = "wasm32")]
            Parent::Sparse { app, owner, .. } => {
                let scope = owner.child_scope()?;
                let view = super::mount(
                    scope.enter(factory),
                    &app.upgrade().map_err(portable::failure)?,
                    &scope,
                )
                .map_err(portable::failure)?;
                Ok(Child::Sparse { view, scope })
            }
        }
    }
    pub(crate) fn attach(&self, child: &Child) -> UiResult {
        // UI construction already places roots; sparse construction is detached.
        match child {
            Child::Ui(_mounted) => Ok(()),
            #[cfg(target_arch = "wasm32")]
            Child::Sparse { .. } => self.place(child, None),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn place(&self, child: &Child, before: Option<&Child>) -> UiResult {
        match (&self.backend, child) {
            (Parent::Ui(_), Child::Ui(child)) => child.place_before(before.and_then(Child::ui)),
            #[cfg(target_arch = "wasm32")]
            (Parent::Sparse { parent, .. }, Child::Sparse { view, .. }) => view
                .place(
                    Some(parent),
                    before.and_then(|before| match before {
                        Child::Sparse { view, .. } => Some(view),
                        _ => None,
                    }),
                )
                .map_err(portable::failure),
            #[cfg(target_arch = "wasm32")]
            _ => Err(glory_ui::Error::Gone),
        }
    }
    pub(crate) fn retain(&self, child: Child) -> UiResult {
        match (&self.backend, child) {
            (Parent::Ui(parent), child) => {
                parent.upgrade().ok_or(glory_ui::Error::Gone)?.retain(child)
            }
            #[cfg(target_arch = "wasm32")]
            (Parent::Sparse { instance, .. }, Child::Sparse { scope, .. }) => {
                instance.retain_child(scope).map_err(portable::failure)
            }
            #[cfg(target_arch = "wasm32")]
            _ => Err(glory_ui::Error::Gone),
        }
    }
    pub(crate) fn report(&self, error: glory_ui::Error) {
        match &self.backend {
            Parent::Ui(parent) => {
                if let Some(scope) = parent.upgrade() {
                    scope.report_error(error);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Parent::Sparse { instance, .. } => instance.fail(super::error(error)),
        }
    }
    pub(crate) fn one<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult {
        let child = self.mount(factory)?;
        self.attach(&child)?;
        self.retain(child)
    }
}

#[doc(hidden)]
pub enum EachRow<O: glory_ui::each::Ownership> {
    Ui(glory_ui::each::UiRow<O>),
    #[cfg(target_arch = "wasm32")]
    Sparse {
        view: super::Mounted,
        _scope: Scope,
        _data: O::Sparse,
    },
}
impl glory_ui::each::Mount for ContentMount {
    type Row<O: glory_ui::each::Ownership> = EachRow<O>;
    fn mount<O: glory_ui::each::Ownership, W: Widget>(
        &self,
        factory: impl FnOnce(&Scope) -> W,
    ) -> glory_ui::Result<EachRow<O>> {
        match &self.backend {
            Parent::Ui(parent) => {
                let parent = parent.upgrade().ok_or(glory_ui::Error::Gone)?;
                Ok(EachRow::Ui(glory_ui::each::Mount::mount::<O, _>(
                    &glory_ui::each::UiMount::new(&parent),
                    factory,
                )?))
            }
            #[cfg(target_arch = "wasm32")]
            Parent::Sparse { app, owner, .. } => {
                let scope = owner.child_scope()?;
                let data = O::sparse(&scope);
                let widget = scope.enter(|| factory(O::sparse_parent(&scope, &data)));
                let view = super::mount(widget, &app.upgrade().map_err(portable::failure)?, &scope)
                    .map_err(portable::failure)?;
                Ok(EachRow::Sparse {
                    view,
                    _scope: scope,
                    _data: data,
                })
            }
        }
    }
    fn place<O: glory_ui::each::Ownership>(
        &self,
        row: &EachRow<O>,
        before: Option<&EachRow<O>>,
    ) -> glory_ui::Result {
        match (&self.backend, row) {
            (Parent::Ui(parent), EachRow::Ui(row)) => {
                let parent = parent.upgrade().ok_or(glory_ui::Error::Gone)?;
                glory_ui::each::Mount::place(
                    &glory_ui::each::UiMount::new(&parent),
                    row,
                    before.and_then(EachRow::ui),
                )
            }
            #[cfg(target_arch = "wasm32")]
            (Parent::Sparse { parent, .. }, EachRow::Sparse { view, .. }) => view
                .place(Some(parent), before.and_then(EachRow::sparse))
                .map_err(portable::failure),
            #[cfg(target_arch = "wasm32")]
            _ => Err(glory_ui::Error::Gone),
        }
    }
    fn has_content<O: glory_ui::each::Ownership>(&self, row: &EachRow<O>) -> bool {
        match (&self.backend, row) {
            (Parent::Ui(parent), EachRow::Ui(row)) => parent.upgrade().is_some_and(|parent| {
                glory_ui::each::Mount::has_content(&glory_ui::each::UiMount::new(&parent), row)
            }),
            #[cfg(target_arch = "wasm32")]
            (Parent::Sparse { .. }, EachRow::Sparse { .. }) => true,
            #[cfg(target_arch = "wasm32")]
            _ => false,
        }
    }
    fn report(&self, error: glory_ui::Error) {
        ContentMount::report(self, error);
    }
}
impl<O: glory_ui::each::Ownership> EachRow<O> {
    fn ui(&self) -> Option<&glory_ui::each::UiRow<O>> {
        match self {
            Self::Ui(row) => Some(row),
            #[cfg(target_arch = "wasm32")]
            _ => None,
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn sparse(&self) -> Option<&super::Mounted> {
        match self {
            Self::Sparse { view, .. } => Some(view),
            _ => None,
        }
    }
}
