use glory_render::{Change, Descriptor, Patch, Props, Role};
use glory_ui::{App, TextKind, Widget, button, check, group, image, input, link, text, view};

#[test]
fn ssr_names_roles_and_clear_preserve_intrinsic_content_and_explicit_group_roles() {
    let html = glory_ssr::render(
        group().fill((
            view().name("Panel & more"),
            view().role(Role::Group),
            text()
                .kind(TextKind::Heading(2))
                .content("Visible heading")
                .name("Heading name"),
            button().label("Button label").name("Button name"),
            input().value("raw🙂").name("Input name"),
            check().label("Check label").name("Check name"),
            image().alt("Description").name("Image name"),
            image().decorative(),
            link()
                .target("#target")
                .label("Link label")
                .name("Link name"),
        )),
    )
    .unwrap();
    assert_eq!(html.matches("role=\"group\"").count(), 2);
    for expected in [
        "Panel &amp; more",
        "Heading name",
        "Button name",
        "Input name",
        "Check name",
        "Image name",
        "Link name",
        "Button label",
        "Check label",
        "Link label",
        "Description",
        "raw🙂",
        "aria-hidden=\"true\"",
    ] {
        assert!(html.contains(expected), "{expected}");
    }

    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = glory_ssr::Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.unwrap();
    let panel = view().name("Panel").mount(app.scope()).unwrap();
    let id = panel.roots()[0];
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains("role=\"group\""));
    app.scope()
        .patch(id, Patch::Role(Change::Set(Some(Role::Status))))
        .unwrap();
    app.scope().patch(id, Patch::Role(Change::Clear)).unwrap();
    app.scope().patch(id, Patch::Name(Change::Keep)).unwrap();
    app.flush().unwrap();
    assert!(renderer.html().unwrap().contains("role=\"group\""));
    app.scope().patch(id, Patch::Name(Change::Clear)).unwrap();
    app.flush().unwrap();
    assert!(
        !renderer.html().unwrap().contains("role=")
            && !renderer.html().unwrap().contains("aria-label")
    );
    let mut invalid = Descriptor::new(Props::Input(Default::default()));
    invalid.role = Some(Role::List);
    let before = app.stats();
    assert!(app.scope().create(invalid).is_err());
    assert_eq!(app.stats(), before);
}
