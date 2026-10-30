use glory_core::Runtime;
use glory_render::{Change, Patch, memory::Memory};
use glory_ui::{App, Widget, button};
use std::rc::Rc;

#[test]
fn shared_graph_has_disjoint_presentation_ids_and_independent_owned_teardown() {
    let runtime = Rc::new(Runtime::new());
    let owner = runtime.scope();
    let label = owner.cage("shared".to_owned());
    let first = App::in_runtime(runtime.clone(), &owner, Memory::new).unwrap();
    let second = App::in_runtime(runtime.clone(), &owner, Memory::new).unwrap();
    first.mount(button().label(label)).unwrap();
    second.mount(button().label(label)).unwrap();
    let a = first.tree().roots()[0];
    let b = second.tree().roots()[0];
    assert_ne!(a, b);
    assert!(
        first
            .scope()
            .patch(b, Patch::Label(Change::Set("wrong".into())))
            .is_err()
    );
    drop(first);
    assert!(label.is_alive());
    label.revise(|text| *text = "updated".into());
    second.flush().unwrap();
    assert_eq!(second.stats().nodes, 1);
    drop(second);
    assert_eq!(runtime.stats().nodes, 1);
    owner.owner().dispose();
    assert_eq!(runtime.stats().nodes, 0);
}

#[test]
fn shared_runtime_only_marks_the_presentation_changed_by_a_binding() {
    let runtime = Rc::new(Runtime::new());
    let owner = runtime.scope();
    let label = owner.cage("initial".to_owned());
    let first = App::in_runtime(runtime.clone(), &owner, Memory::new).unwrap();
    let second = App::in_runtime(runtime.clone(), &owner, Memory::new).unwrap();
    first.mount(button().label(label)).unwrap();
    second.mount(button().label("static")).unwrap();
    assert!(!first.has_pending_presentation());
    assert!(!second.has_pending_presentation());
    label.revise(|label| *label = "changed".into());
    runtime.flush().unwrap();
    assert!(first.has_pending_presentation());
    assert!(!second.has_pending_presentation());
    first.pump().unwrap();
    assert!(!first.has_pending_presentation());
    let mounted = button().mount(first.scope()).unwrap();
    assert!(first.has_pending_presentation());
    first.flush().unwrap();
    drop(mounted);
    assert!(first.has_pending_presentation());
    first.flush().unwrap();
    assert!(!first.has_pending_presentation());
}

#[test]
fn composed_widgets_keep_build_resources_until_their_scope_retires() {
    use glory_ui::{Result, Scope, Widget};
    use std::cell::Cell;
    struct Leaf(Rc<Cell<usize>>);
    impl Drop for Leaf {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    impl Widget for Leaf {
        fn build(&mut self, _: &Scope) -> Result {
            Ok(())
        }
    }
    struct Function(Option<Leaf>);
    impl Widget for Function {
        fn compose(&mut self, _: &glory_core::Scope) -> Result<Option<Box<dyn Widget>>> {
            Ok(self.0.take().map(|leaf| Box::new(leaf) as Box<dyn Widget>))
        }
    }
    let dropped = Rc::new(Cell::new(0));
    let app = App::memory();
    app.mount(Function(Some(Leaf(dropped.clone())))).unwrap();
    assert_eq!(dropped.get(), 0);
    app.clear().unwrap();
    assert_eq!(dropped.get(), 1);

    app.mount(Some(Leaf(dropped.clone()))).unwrap();
    assert_eq!(dropped.get(), 1, "optional Widget must retain its child");
    app.clear().unwrap();
    assert_eq!(dropped.get(), 2);
    app.mount(None::<Leaf>).unwrap();
    assert!(app.tree().is_empty());
    app.clear().unwrap();
    assert_eq!(dropped.get(), 2);
}
