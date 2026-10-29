use glory_core::Runtime;
use glory_render::{Change, Patch, memory::Memory};
use glory_ui::{App, button};
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
