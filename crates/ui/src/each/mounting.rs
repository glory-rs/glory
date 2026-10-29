use super::*;

#[derive(Clone)]
pub struct UiMount(crate::WeakScope);
pub struct UiRow<O: Ownership> {
    mounted: crate::Mounted,
    _data: O::Ui,
}
impl UiMount {
    pub fn new(scope: &crate::Scope) -> Self {
        Self(scope.downgrade())
    }
}
impl Mount for UiMount {
    type Row<O: Ownership> = UiRow<O>;
    fn mount<O: Ownership, W: Widget>(
        &self,
        factory: impl FnOnce(&CoreScope) -> W,
    ) -> Result<UiRow<O>> {
        let parent = self.0.upgrade().ok_or(crate::Error::Gone)?;
        let data = O::ui(&parent)?;
        let parent = O::ui_parent(&parent, &data);
        let mounted = parent.mount_scoped(factory)?;
        Ok(UiRow {
            mounted,
            _data: data,
        })
    }
    fn place<O: Ownership>(&self, row: &UiRow<O>, before: Option<&UiRow<O>>) -> Result {
        row.mounted.place_before(before.map(|row| &row.mounted))
    }
    fn has_content<O: Ownership>(&self, row: &UiRow<O>) -> bool {
        !row.mounted.roots().is_empty()
    }
    fn report(&self, error: crate::Error) {
        if let Some(scope) = self.0.upgrade() {
            scope.report_error(error);
        }
    }
    fn clear_on_error(&self) -> bool {
        true
    }
}

/// Monomorphized row ownership: keyed/seed rows carry zero data-owner storage.
#[doc(hidden)]
pub trait Ownership: 'static {
    type Ui: 'static;
    type Sparse: 'static;
    fn ui(parent: &crate::Scope) -> Result<Self::Ui>;
    fn ui_parent<'a>(parent: &'a crate::Scope, data: &'a Self::Ui) -> &'a crate::Scope;
    fn sparse(parent: &CoreScope) -> Self::Sparse;
    fn sparse_parent<'a>(parent: &'a CoreScope, data: &'a Self::Sparse) -> &'a CoreScope;
}
pub struct Direct;
pub struct Reactive;
impl Ownership for Direct {
    type Ui = ();
    type Sparse = ();
    fn ui(_: &crate::Scope) -> Result<()> {
        Ok(())
    }
    fn ui_parent<'a>(parent: &'a crate::Scope, _: &'a ()) -> &'a crate::Scope {
        parent
    }
    fn sparse(_: &CoreScope) {}
    fn sparse_parent<'a>(parent: &'a CoreScope, _: &'a ()) -> &'a CoreScope {
        parent
    }
}
impl Ownership for Reactive {
    type Ui = crate::Scope;
    type Sparse = CoreScope;
    fn ui(parent: &crate::Scope) -> Result<crate::Scope> {
        parent.child()
    }
    fn ui_parent<'a>(_: &'a crate::Scope, data: &'a crate::Scope) -> &'a crate::Scope {
        data
    }
    fn sparse(parent: &CoreScope) -> CoreScope {
        parent.child()
    }
    fn sparse_parent<'a>(_: &'a CoreScope, data: &'a CoreScope) -> &'a CoreScope {
        data
    }
}
