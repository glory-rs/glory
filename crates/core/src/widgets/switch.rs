use std::fmt;

use crate::reflow::Record;
use crate::{Scope, View, ViewFactory, ViewId, Widget};

pub struct Case {
    pub cond: Box<dyn Record<bool>>,
    pub tmpl: Box<dyn ViewFactory>,
    use_cache: bool,
    cached_view: Option<View>,
}
impl Case {
    pub fn new<T>(cond: impl Record<bool> + 'static, tmpl: T) -> Self
    where
        T: ViewFactory + 'static,
    {
        Self {
            cond: Box::new(cond),
            tmpl: Box::new(tmpl),
            use_cache: false,
            cached_view: None,
        }
    }

    pub fn cache(mut self, value: bool) -> Self {
        self.use_cache = value;
        self
    }
}

#[derive(Default)]
pub struct Switch {
    cases: Vec<Case>,
    active_index: Option<usize>,
    active_view_id: Option<ViewId>,
}
impl fmt::Debug for Switch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Switch").field("cases.len", &self.cases.len()).finish()
    }
}

impl Switch {
    pub fn new() -> Self {
        Self {
            cases: Vec::new(),
            active_index: None,
            active_view_id: None,
        }
    }
    pub fn case<T>(self, cond: impl Record<bool> + 'static, tmpl: T) -> Self
    where
        T: ViewFactory + 'static,
    {
        self.push(Case::new(cond, tmpl))
    }
    pub fn push(mut self, case: Case) -> Self {
        self.cases.push(case);
        self
    }
}

impl Widget for Switch {
    fn build(&mut self, ctx: &mut Scope) {
        for (index, case) in self.cases.iter().enumerate() {
            case.cond.bind_view(ctx.view_id());
            if *case.cond.get() {
                let view_id = case.tmpl.make_view(ctx);
                self.active_index = Some(index);
                self.active_view_id = Some(view_id.clone());
                ctx.attach_child(&view_id);
                return;
            }
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        let mut index = None;
        for (i, case) in self.cases.iter().enumerate() {
            if *case.cond.get() {
                index = Some(i);
                break;
            }
        }
        if index != self.active_index {
            if let Some(active_view_id) = &self.active_view_id {
                if let (Some(active_index), Some(active_view)) = (self.active_index, ctx.detach_child(active_view_id)) {
                    let active_case = self.cases.get_mut(active_index).unwrap();
                    if active_case.use_cache {
                        active_case.cached_view = Some(active_view);
                    }
                }
            }
            self.active_index = index;
            if let Some(index) = index {
                let case = self.cases.get_mut(index).unwrap();
                let view_id = if let Some(view) = &case.cached_view {
                    view.id.clone()
                } else {
                    case.tmpl.make_view(ctx)
                };
                self.active_view_id = Some(view_id.clone());
                ctx.attach_child(&view_id);
            }
        }
    }
}
