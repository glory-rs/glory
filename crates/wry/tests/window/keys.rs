use glory_render::{
    Direction, Event, EventKind, FocusMode, Role, ScrollAxes, Semantics, TagSeparators,
    TagsCommand, TreeCommand, TreeItem, TreeNavigation,
};
use glory_ui::{button, group, input, text, view};
use glory_wry::shell::WindowApp;

pub(super) fn mount(app: &WindowApp) -> glory_ui::Result {
    use glory_render::{
        Action, ChoiceCommand, ComboboxKind, OptionSemantics, Orientation, PopupKind, SemanticId,
    };
    let list_id = SemanticId::new("key-list")?;
    let target = app.app().scope().cage(None);
    let scope = app.app().scope().downgrade();
    app.mount(
        group().fill((
            button()
                .label("Choose result")
                .class("key-combo")
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("key-combo")?),
                    controls: Some(list_id.clone()),
                    combobox: Some(ComboboxKind::Select),
                    popup: Some(PopupKind::Listbox),
                    expanded: Some(true),
                    ..Default::default()
                })))
                .on(EventKind::Choice, move |event| {
                    if let Event::Choice {
                        command: ChoiceCommand::Last,
                        ..
                    } = event
                    {
                        scope
                            .upgrade()
                            .unwrap()
                            .action(Action::RevealOption {
                                node: target.get().unwrap(),
                            })
                            .unwrap();
                    }
                }),
            view()
                .class("key-list")
                .role(Role::Listbox)
                .semantics(Some(Box::new(Semantics {
                    id: Some(list_id.clone()),
                    navigation: Some(Orientation::Vertical),
                    ..Default::default()
                })))
                .fill(
                    button()
                        .label("Last result")
                        .node_ref(target)
                        .focus(FocusMode::Programmatic)
                        .semantics(Some(Box::new(Semantics {
                            id: Some(SemanticId::new("key-last")?),
                            option: Some(OptionSemantics { owner: list_id }),
                            selected: Some(false),
                            ..Default::default()
                        }))),
                ),
        )),
    )?;
    let menu = app.app().scope().cage(None);
    let scope = app.app().scope().downgrade();
    let trigger_id = SemanticId::new("key-menu-trigger")?;
    app.mount(
        group().fill((
            button()
                .label("Leave closed menu")
                .class("key-menu-trigger")
                .semantics(Some(Box::new(Semantics {
                    id: Some(trigger_id.clone()),
                    ..Default::default()
                })))
                .on(EventKind::Activate, move |_| {
                    scope
                        .upgrade()
                        .unwrap()
                        .action(Action::ExitMenu {
                            node: menu.get().unwrap(),
                            backwards: false,
                        })
                        .unwrap()
                }),
            view()
                .role(Role::Menu)
                .focus(FocusMode::Programmatic)
                .node_ref(menu)
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("key-closed-menu")?),
                    hidden: true,
                    navigation: Some(Orientation::Vertical),
                    layer: Some(glory_render::Layer {
                        modal: false,
                        return_focus: Some(trigger_id),
                        ..Default::default()
                    }),
                    ..Default::default()
                }))),
            button().class("key-menu-next").label("Next"),
        )),
    )?;
    app.mount(
        glory_ui::list::list(
            500usize,
            |index| index,
            |_, index| button().label(format!("Virtual row {index}")),
        )
        .name("Keyboard window")
        .height(80.)
        .row_height(20.),
    )?;
    let last = app.app().scope().cage(String::new());
    let day = app.app().scope().cage(None);
    app.mount(
        group().fill((
            view()
                .role(Role::Grid)
                .semantics(Some(Box::new(Semantics {
                    grid_navigation: Some(Direction::Rtl),
                    ..Default::default()
                })))
                .on(EventKind::GridNavigate, move |event| {
                    if let Event::GridNavigate { from, command } = event {
                        assert_eq!(day.get(), Some(from));
                        last.revise(|s| *s = format!("grid {command:?}"));
                    }
                })
                .fill(group().fill((
                    button().class("key-day").label("29").node_ref(day),
                    input().class("key-grid-editor"),
                ))),
            view()
                .role(Role::Tree)
                .semantics(Some(Box::new(Semantics {
                    tree_navigation: Some(TreeNavigation::default()),
                    ..Default::default()
                })))
                .on(EventKind::TreeNavigate, move |event| {
                    if let Event::TreeNavigate {
                        command: TreeCommand::Search { text, .. },
                        ..
                    } = event
                    {
                        last.revise(|s| *s = format!("tree {text}"));
                    }
                })
                .fill(
                    view()
                        .role(Role::TreeItem)
                        .name("Branch")
                        .class("key-branch")
                        .focus(FocusMode::TabStop)
                        .semantics(Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 1,
                                position: 0,
                                count: 1,
                            }),
                            selected: Some(false),
                            ..Default::default()
                        }))),
                ),
            view()
                .class("key-scroll")
                .name("Vertical scroller")
                .focus(FocusMode::TabStop)
                .semantics(Some(Box::new(Semantics {
                    scroll_keyboard: Some(ScrollAxes::Vertical),
                    ..Default::default()
                })))
                .on(EventKind::ScrollKeyboard, move |event| {
                    if let Event::ScrollKeyboard { command, .. } = event {
                        last.revise(|s| *s = format!("scroll {:?}", command.movement));
                    }
                }),
            input()
                .class("key-tags")
                .value("text")
                .semantics(Some(Box::new(Semantics {
                    tags_input: Some(TagSeparators::default()),
                    ..Default::default()
                })))
                .on(EventKind::Tags, move |event| {
                    if let Event::Tags { command, .. } = event {
                        let message = match command {
                            TagsCommand::Commit { value } => format!("commit {value}"),
                            TagsCommand::Paste {
                                text, start, end, ..
                            } => format!("paste {start}-{end} {text}"),
                            _ => String::new(),
                        };
                        last.revise(|s| *s = message);
                    }
                }),
            text().class("key-receipt").content(last),
        )),
    )?;
    Ok(())
}
