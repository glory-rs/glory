use glory::prelude::*;
use glory::render::conformance::flex_cases;

struct Case(usize);
impl Widget for Case {
    fn build(&mut self, scope: &glory::ui::Scope) -> glory::ui::Result {
        let cases = flex_cases();
        let case = cases.get(self.0).ok_or(glory::ui::Error::Gone)?;
        let parent = scope.create(case.parent_descriptor())?;
        let child = scope.child_in(parent)?;
        for _ in 0..2 {
            child.create(case.child_descriptor())?;
        }
        scope.retain(child)?;
        Ok(())
    }
}
pub fn ui(index: usize) -> impl Widget {
    view()
        .class("flex-frame")
        .fill((Case(index), input().class("flex-input").value("initial🙂")))
}
