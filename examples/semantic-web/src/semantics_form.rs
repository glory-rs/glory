use glory::prelude::*;

/// The same Rust builders produce live DOM and the server adoption fixture.
pub fn ui() -> impl Widget {
    let roles = [
        Role::Group,
        Role::Main,
        Role::Navigation,
        Role::Section,
        Role::Status,
        Role::Alert,
    ];
    let regions: Vec<_> = roles
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            view()
                .class(format!("role-{index}"))
                .role(role)
                .name(format!("Region {index}"))
                .fill(text().content("Region content"))
        })
        .collect();
    view().class("sem-root").fill((
        view()
            .class("sem-panel")
            .name("Panel")
            .fill(text().content("Panel content")),
        view()
            .class("sem-group")
            .role(Role::Group)
            .name("Explicit group"),
        text()
            .class("sem-heading")
            .kind(TextKind::Heading(2))
            .content("Visible heading")
            .name("Heading name"),
        text().class("sem-inline").content("Inline content"),
        text()
            .class("sem-paragraph")
            .kind(TextKind::Paragraph)
            .content("Paragraph content"),
        button()
            .class("sem-button")
            .label("Button label")
            .name("Button name"),
        input().class("sem-input").value("a🙂z").name("Editor name"),
        check()
            .class("sem-check")
            .label("Check label")
            .name("Check name"),
        image()
            .class("sem-image")
            .alt("Image description")
            .name("Image name"),
        image().class("sem-decorative").decorative(),
        link()
            .class("sem-link")
            .target("#target")
            .label("Link label")
            .name("Link name"),
        view().class("sem-regions").fill(regions).fill(
            view()
                .class("sem-list")
                .role(Role::List)
                .name("Collection")
                .fill(
                    view()
                        .class("sem-item")
                        .role(Role::ListItem)
                        .item(Some(ItemPosition {
                            index: 12,
                            count: Some(100_000_000),
                        }))
                        .name("Logical item"),
                ),
        ),
    ))
}
