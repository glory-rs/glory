//! Statically selected host adapters for the shared Widget/content controllers.
use super::*;

type UiResult<T = ()> = glory_ui::Result<T>;

/// Internal operations for named configuration actions, not a component API.
#[doc(hidden)]
pub trait Mount {
    fn text<V: TextValue + 'static>(&self, field: Field, value: V) -> UiResult;
    fn flag<V: BoolValue>(&self, field: Field, value: V) -> UiResult;
    fn on(&self, click: Click, callback: impl FnMut() + 'static) -> UiResult;
    fn child<C: Content>(&self, outlet: Outlet, content: C) -> UiResult;
}

impl Mount for portable::Mount<'_> {
    fn text<V: TextValue + 'static>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.frame.slot(field.0)?.0;
        if value.reactive_text() {
            let frame = self.frame.clone();
            let weak = self.scope.downgrade();
            self.scope.binding(move || {
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
            self.frame.text(
                slot,
                glory_core::untracked(|| value.read_text()).map_err(portable::failure)?,
            )?;
            Ok(())
        }
    }
    fn flag<V: BoolValue>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.frame.slot(field.0)?.0;
        if value.reactive() {
            let frame = self.frame.clone();
            let weak = self.scope.downgrade();
            self.scope.binding(move || {
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
            self.frame.flag(
                slot,
                glory_core::untracked(|| value.read_bool()).map_err(portable::failure)?,
            )?;
            Ok(())
        }
    }
    fn on(&self, click: Click, callback: impl FnMut() + 'static) -> UiResult {
        self.handlers
            .borrow_mut()
            .insert(self.frame.slot(click.0)?.0, Box::new(callback));
        Ok(())
    }
    fn child<C: Content>(&self, outlet: Outlet, content: C) -> UiResult {
        let scope = self.outlet(outlet)?;
        content.install(&scope, &UiContent(scope.downgrade()))
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct SparseMount<'a> {
    pub definition: &'a Compiled,
    pub app: &'a crate::template::App,
    pub scope: &'a Scope,
    pub instance: &'a crate::template::Instance,
}
#[cfg(target_arch = "wasm32")]
impl SparseMount<'_> {
    fn slot(&self, name: &str) -> UiResult<u16> {
        self.definition
            .slot(name)
            .map(|(slot, _)| slot)
            .ok_or(glory_ui::Error::Gone)
    }
}
#[cfg(target_arch = "wasm32")]
impl Mount for SparseMount<'_> {
    fn text<V: TextValue + 'static>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.slot(field.0)?;
        if value.reactive_text() {
            self.instance
                .try_bind_values(self.scope, [slot], move || Ok([value.read_text()?]))
                .map_err(portable::failure)
        } else {
            self.instance
                .set(
                    slot,
                    glory_core::untracked(|| value.read_text()).map_err(portable::failure)?,
                )
                .map_err(portable::failure)
        }
    }
    fn flag<V: BoolValue>(&self, field: Field, mut value: V) -> UiResult {
        let slot = self.slot(field.0)?;
        if value.reactive() {
            self.instance
                .bind_flag(self.scope, slot, move || value.read_bool())
                .map_err(portable::failure)
        } else {
            let next = value.read_bool().map_err(portable::failure)?;
            if next
                != self
                    .instance
                    .flag_initial(slot)
                    .map_err(portable::failure)?
            {
                self.instance
                    .set_flag(slot, next)
                    .map_err(portable::failure)?;
            }
            Ok(())
        }
    }
    fn on(&self, click: Click, callback: impl FnMut() + 'static) -> UiResult {
        self.instance
            .on_click(self.slot(click.0)?, callback)
            .map_err(portable::failure)
    }
    fn child<C: Content>(&self, outlet: Outlet, content: C) -> UiResult {
        let parent = self
            .instance
            .children(self.slot(outlet.0)?)
            .map_err(portable::failure)?;
        content.install(
            self.scope,
            &SparseContent {
                app: self.app.downgrade(),
                owner: self.scope.id(),
                parent,
                instance: self.instance.clone(),
            },
        )
    }
}

/// Host-specific storage; Each/Show/Content keep one shared implementation.
#[doc(hidden)]
pub trait ContentMount: glory_ui::each::Mount {
    type Child: 'static;
    fn mount_widget<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult<Self::Child>;
    fn attach(&self, child: &Self::Child) -> UiResult;
    fn retain(&self, child: Self::Child) -> UiResult;
    fn report_error(&self, error: glory_ui::Error);
    fn one<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult {
        let child = self.mount_widget(factory)?;
        self.attach(&child)?;
        self.retain(child)
    }
}

#[derive(Clone)]
pub(crate) struct UiContent(glory_ui::WeakScope);
impl ContentMount for UiContent {
    type Child = glory_ui::Mounted;
    fn mount_widget<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult<Self::Child> {
        self.0
            .upgrade()
            .ok_or(glory_ui::Error::Gone)?
            .mount_with(factory)
    }
    fn attach(&self, _: &Self::Child) -> UiResult {
        Ok(())
    }
    fn retain(&self, child: Self::Child) -> UiResult {
        self.0.upgrade().ok_or(glory_ui::Error::Gone)?.retain(child)
    }
    fn report_error(&self, error: glory_ui::Error) {
        if let Some(scope) = self.0.upgrade() {
            scope.report_error(error);
        }
    }
}
impl glory_ui::each::Mount for UiContent {
    type Row<O: glory_ui::each::Ownership> = glory_ui::each::UiRow<O>;
    fn mount<O: glory_ui::each::Ownership, W: Widget>(
        &self,
        factory: impl FnOnce(&Scope) -> W,
    ) -> UiResult<Self::Row<O>> {
        let scope = self.0.upgrade().ok_or(glory_ui::Error::Gone)?;
        glory_ui::each::Mount::mount::<O, W>(&glory_ui::each::UiMount::new(&scope), factory)
    }
    fn place<O: glory_ui::each::Ownership>(
        &self,
        row: &Self::Row<O>,
        before: Option<&Self::Row<O>>,
    ) -> UiResult {
        let scope = self.0.upgrade().ok_or(glory_ui::Error::Gone)?;
        glory_ui::each::Mount::place(&glory_ui::each::UiMount::new(&scope), row, before)
    }
    fn has_content<O: glory_ui::each::Ownership>(&self, row: &Self::Row<O>) -> bool {
        self.0.upgrade().is_some_and(|scope| {
            glory_ui::each::Mount::has_content(&glory_ui::each::UiMount::new(&scope), row)
        })
    }
    fn report(&self, error: glory_ui::Error) {
        self.report_error(error);
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub(crate) struct SparseContent {
    app: crate::template::WeakApp,
    owner: glory_core::InstanceId,
    parent: crate::template::Children,
    instance: crate::template::Instance,
}
#[cfg(target_arch = "wasm32")]
pub(crate) struct SparseChild {
    view: super::Mounted,
    scope: Scope,
}
#[cfg(target_arch = "wasm32")]
impl ContentMount for SparseContent {
    type Child = SparseChild;
    fn mount_widget<W: Widget>(&self, factory: impl FnOnce() -> W) -> UiResult<Self::Child> {
        let scope = self.owner.child_scope()?;
        let view = super::mount(
            scope.enter(factory),
            &self.app.upgrade().map_err(portable::failure)?,
            &scope,
        )
        .map_err(portable::failure)?;
        Ok(SparseChild { view, scope })
    }
    fn attach(&self, child: &Self::Child) -> UiResult {
        child
            .view
            .place(Some(&self.parent), None)
            .map_err(portable::failure)
    }
    fn retain(&self, child: Self::Child) -> UiResult {
        self.instance
            .retain_child(child.scope)
            .map_err(portable::failure)
    }
    fn report_error(&self, error: glory_ui::Error) {
        self.instance.fail(super::error(error));
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) struct SparseRow<O: glory_ui::each::Ownership> {
    view: super::Mounted,
    _scope: Scope,
    _data: O::Sparse,
}
#[cfg(target_arch = "wasm32")]
impl glory_ui::each::Mount for SparseContent {
    type Row<O: glory_ui::each::Ownership> = SparseRow<O>;
    fn mount<O: glory_ui::each::Ownership, W: Widget>(
        &self,
        factory: impl FnOnce(&Scope) -> W,
    ) -> UiResult<Self::Row<O>> {
        let scope = self.owner.child_scope()?;
        let data = O::sparse(&scope);
        let widget = scope.enter(|| factory(O::sparse_parent(&scope, &data)));
        let view = super::mount(
            widget,
            &self.app.upgrade().map_err(portable::failure)?,
            &scope,
        )
        .map_err(portable::failure)?;
        Ok(SparseRow {
            view,
            _scope: scope,
            _data: data,
        })
    }
    fn place<O: glory_ui::each::Ownership>(
        &self,
        row: &Self::Row<O>,
        before: Option<&Self::Row<O>>,
    ) -> UiResult {
        row.view
            .place(Some(&self.parent), before.map(|row| &row.view))
            .map_err(portable::failure)
    }
    fn has_content<O: glory_ui::each::Ownership>(&self, _: &Self::Row<O>) -> bool {
        true
    }
    fn report(&self, error: glory_ui::Error) {
        self.report_error(error);
    }
}
