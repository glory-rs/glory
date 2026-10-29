//! Acceptance-only exports are excluded from measured artifacts.
use glory::html::view::{Field, Node, Outlet, Template, children as outlet_children};
use glory::web::template::App;
use wasm_bindgen::prelude::*;

const fn children<const N: usize>() -> [Node; N] {
    let mut values = [Node::Element {
        tag: "span",
        attrs: &[],
        children: &[],
    }; N];
    values[0] = Node::Field(Field::new("value"));
    values
}
static SMALL: Template = Template::new(&[Node::Element {
    tag: "div",
    attrs: &[outlet_children(Outlet::new("children"))],
    children: &children::<1>(),
}]);
static LARGE: Template = Template::new(&[Node::Element {
    tag: "div",
    attrs: &[outlet_children(Outlet::new("children"))],
    children: &children::<200>(),
}]);
#[wasm_bindgen]
pub fn probe_templates() -> String {
    let doc = web_sys::window().unwrap().document().unwrap();
    let mut counts = Vec::new();
    for template in [&SMALL, &LARGE] {
        let root = doc.create_element("div").unwrap();
        let app = App::new(root.clone()).unwrap();
        let owner = app.scope();
        assert!(App::new(root.clone()).is_err());
        let definition = template.cached().unwrap();
        let text = definition.slot("value").unwrap().0;
        let outlet = definition.slot("children").unwrap().0;
        let instance = app.mount(&owner, template).unwrap();
        assert!(app.mount(&owner, template).is_err());
        instance.place(None, None).unwrap();
        let value = owner.cage("数据🙂𠮷\n\"\\".to_owned());
        instance.bind(&owner, text, value).unwrap();
        assert!(instance.set(outlet, "wrong".into()).is_err());
        assert!(instance.set(text, "x".repeat(65537)).is_err());
        assert!(
            instance
                .bind_values(&owner, [text, text], || ["a".into(), "b".into()])
                .is_err()
        );
        app.flush().unwrap();
        assert_eq!(root.text_content().unwrap(), "数据🙂𠮷\n\"\\");
        let physical = root.first_child().unwrap();
        let static_last = physical.last_child().unwrap();
        let stats = app.stats();
        counts.push(format!(
            "[{}, {}, {}]",
            stats.instances, stats.targets, stats.records
        ));
        assert_eq!((stats.instances, stats.targets, stats.records), (1, 2, 1));
        value.revise(|v| *v = "updated汉字🦀".into());
        app.flush().unwrap();
        assert_eq!(root.text_content().unwrap(), "updated汉字🦀");
        assert!(root.first_child().unwrap().is_same_node(Some(&physical)));
        assert!(
            physical
                .last_child()
                .unwrap()
                .is_same_node(Some(&static_last))
        );
        let other_root = doc.create_element("div").unwrap();
        let other = App::new(other_root).unwrap();
        let foreign = other.scope();
        assert!(app.mount(&foreign, template).is_err());
        assert!(instance.bind(&foreign, text, "constant").is_err());
        let foreign_instance = other.mount(&foreign, &SMALL).unwrap();
        assert!(
            foreign_instance
                .place(Some(&instance.children(outlet).unwrap()), None)
                .is_err()
        );
        assert!(instance.place(None, Some(&foreign_instance)).is_err());
        let child = owner.child();
        let row = app.mount(&child, &SMALL).unwrap();
        assert!(row.place(None, None).is_err());
        row.place(Some(&instance.children(outlet).unwrap()), None)
            .unwrap();
        row.set(text, "child".into()).unwrap();
        app.flush().unwrap();
        assert_eq!(app.stats().instances, 2);
        drop(child);
        app.flush().unwrap();
        assert_eq!(app.stats().instances, 1);
        assert!(row.set(text, "stale".into()).is_err());
        for _ in 0..32 {
            let child = owner.child();
            let row = app.mount(&child, &SMALL).unwrap();
            row.place(Some(&instance.children(outlet).unwrap()), None)
                .unwrap();
            app.flush().unwrap();
            drop(child);
            app.flush().unwrap();
        }
        assert_eq!(app.stats().records, 1);
        assert_eq!(app.runtime().stats().owners, 2);
        app.close().unwrap();
        assert_eq!(app.stats().records, 0);
        assert_eq!(app.runtime().stats().nodes, 0);
        assert!(root.first_child().is_none());
        assert!(instance.set(text, "stale".into()).is_err());
        let weak = app.downgrade();
        drop(app);
        assert!(weak.upgrade().is_err());
        assert!(App::new(root).is_ok());
    }
    format!(
        "{{\"static_nodes\":[2,201],\"live\":[{}]}}",
        counts.join(",")
    )
}
