//! Host-independent branch selection and owned retirement.
#[doc(hidden)]
pub struct Branch<M> {
    selected: Option<bool>,
    mounted: Option<M>,
}
impl<M> Default for Branch<M> {
    fn default() -> Self {
        Self {
            selected: None,
            mounted: None,
        }
    }
}
impl<M> Branch<M> {
    pub fn update<E>(
        &mut self,
        next: bool,
        mount: impl FnOnce(bool) -> Result<Option<M>, E>,
    ) -> Result<(), E> {
        if self.selected == Some(next) {
            return Ok(());
        }
        self.selected = None;
        drop(self.mounted.take());
        self.mounted = mount(next)?;
        self.selected = Some(next);
        Ok(())
    }
}
