use std::cell::RefCell;
use std::rc::Rc;

use glory_core::Vary;
use glory_render::{Descriptor, Props};

use crate::app::{Error, Mounted, Result, Scope, Widget};

type Factory = Box<dyn FnMut() -> Box<dyn Widget>>;
pub struct When {
    condition: Option<Vary<bool>>,
    yes: Option<Factory>,
    no: Option<Factory>,
}
pub fn when<W: Widget>(
    condition: impl Into<Vary<bool>>,
    mut yes: impl FnMut() -> W + 'static,
) -> When {
    When {
        condition: Some(condition.into()),
        yes: Some(Box::new(move || Box::new(yes()))),
        no: None,
    }
}
impl When {
    pub fn otherwise<W: Widget>(mut self, mut no: impl FnMut() -> W + 'static) -> Self {
        self.no = Some(Box::new(move || Box::new(no())));
        self
    }
}
struct Branch {
    state: crate::branch::Branch<Mounted>,
    yes: Factory,
    no: Option<Factory>,
}
impl Widget for When {
    fn build(&mut self, scope: &Scope) -> Result {
        let anchor = scope.create(Descriptor::new(Props::Group))?;
        let content = scope.child_in(anchor)?;
        let parent = content.weak();
        let session = Rc::downgrade(&scope.session()?);
        let condition = self.condition.take().ok_or(Error::Gone)?;
        let state = Rc::new(RefCell::new(Branch {
            state: Default::default(),
            yes: self.yes.take().ok_or(Error::Gone)?,
            no: self.no.take(),
        }));
        scope.retain(content)?;
        scope.retain(state.clone())?;
        scope.structural(move || {
            let Some(session) = session.upgrade() else {
                return;
            };
            let Some(parent) = parent.upgrade() else {
                return;
            };
            let show = match condition.try_get() {
                Ok(show) => show,
                Err(error) => {
                    session.report(error.into());
                    return;
                }
            };
            let mut state = state.borrow_mut();
            let Branch { state, yes, no } = &mut *state;
            if let Err(error) = state.update(show, |show| {
                if show {
                    parent.mount_with(yes).map(Some)
                } else {
                    no.as_mut()
                        .map(|factory| parent.mount_with(factory))
                        .transpose()
                }
            }) {
                session.report(error);
            }
        });
        Ok(())
    }
}
