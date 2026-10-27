//! Opt-in native protocol view; all state and menu policy belong to this app.
use glory::prelude::*;
use glory::render::{
    Action, AnchorAlign, AnchorPlacement, AnchorSide, ContextEditPolicy, EventKind, Layer,
    LayerFocus, LayerPlacement, MenuCommand, MenuItemKind, MenuItemSemantics, Orientation,
    PopupKind,
};
const LABELS: [&str; 3] = ["Alpha action", "Disabled action", "中文 action"];
pub(crate) const CSS: &str = ".menu-probe{display:flex;flex-direction:column;align-items:flex-start;gap:16px;padding:24px;font:18px sans-serif}.menu-bar{display:flex;flex-direction:row;gap:8px}.menu-panel{display:flex;flex-direction:column;gap:4px;padding:8px;width:260px;background:#f5f5f5;color:#202020;border:1px solid #888}.menu-panel button{height:36px;text-align:left}.menu-context{margin-top:160px;padding:12px;width:280px;height:70px;border:1px solid #888}.menu-palette{display:flex;flex-direction:column;gap:12px;width:280px;padding:16px;background:white}";
fn name(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let open = scope.cage(true);
        let active = scope.cage(0usize);
        let current = scope.cage(0usize);
        let palette = scope.cage(false);
        let checked = scope.cage(true);
        let draft = scope.cage(String::new());
        let context = scope.cage(false);
        let point = scope.cage(LayerPlacement::Anchor(AnchorPlacement::new(
            AnchorSide::Bottom,
            AnchorAlign::Start,
        )));
        let panel = scope.cage(None);
        let roots = [scope.cage(None), scope.cage(None)];
        let options = [scope.cage(None), scope.cage(None), scope.cage(None)];
        let check_ref = scope.cage(None);
        let mut triggers: Vec<Box<dyn Widget>> = Vec::new();
        for (i, label) in ["File", "Edit"].into_iter().enumerate() {
            let weak = scope.downgrade();
            triggers.push(Box::new(
                button()
                    .label(label)
                    .node_ref(roots[i])
                    .focus(active.map(move |active| {
                        if *active == i {
                            FocusMode::TabStop
                        } else {
                            FocusMode::Programmatic
                        }
                    }))
                    .style(Style::new().width(Length::Px(100.)).height(Length::Px(40.)))
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(name(if i == 0 { "menu-file" } else { "menu-edit" })),
                            menubar_item: true,
                            navigation_item: true,
                            menu_item: Some(MenuItemSemantics {
                                kind: MenuItemKind::Action,
                                owner: name("menu-bar"),
                            }),
                            popup: Some(PopupKind::Menu),
                            controls: Some(name("menu-panel")),
                            expanded: Some(open.get() && !context.get() && active.get() == i),
                            ..Default::default()
                        }))
                    }))
                    .on(EventKind::Activate, move |_| {
                        let was_open = open.get() && !context.get() && active.get() == i;
                        active.revise(|v| *v = i);
                        context.revise(|v| *v = false);
                        point.revise(|v| {
                            *v = LayerPlacement::Anchor(AnchorPlacement::new(
                                AnchorSide::Bottom,
                                AnchorAlign::Start,
                            ))
                        });
                        open.revise(|v| *v = !was_open);
                    })
                    .on(EventKind::Menu, move |event| {
                        if let Event::Menu { command, .. } = event
                            && matches!(command, MenuCommand::First | MenuCommand::Last)
                        {
                            active.revise(|v| *v = i);
                            context.revise(|v| *v = false);
                            point.revise(|v| {
                                *v = LayerPlacement::Anchor(AnchorPlacement::new(
                                    AnchorSide::Bottom,
                                    AnchorAlign::Start,
                                ))
                            });
                            open.revise(|v| *v = true);
                            let index = if command == MenuCommand::Last { 3 } else { 0 };
                            current.revise(|v| *v = index);
                            if let Some(scope) = weak.upgrade()
                                && let Some(node) = if index == 3 {
                                    check_ref.get()
                                } else {
                                    options[index].get()
                                }
                            {
                                let _ = scope.action(Action::Focus(node));
                            }
                        }
                    }),
            ));
        }
        let mut items: Vec<Box<dyn Widget>> = Vec::new();
        for (i, label) in LABELS.into_iter().enumerate() {
            items.push(Box::new(
                button()
                    .label(label)
                    .enabled(i != 1)
                    .node_ref(options[i])
                    .focus(current.map(move |current| {
                        if *current == i {
                            FocusMode::TabStop
                        } else {
                            FocusMode::Programmatic
                        }
                    }))
                    .semantics(Some(Box::new(Semantics {
                        navigation_item: true,
                        menu_item: Some(MenuItemSemantics {
                            kind: MenuItemKind::Action,
                            owner: name("menu-panel"),
                        }),
                        ..Default::default()
                    })))
                    .on(EventKind::FocusChanged, move |event| {
                        if event == Event::FocusChanged(true) {
                            current.revise(|v| *v = i);
                        }
                    })
                    .on(EventKind::Activate, move |_| open.revise(|v| *v = false)),
            ));
        }
        let weak = scope.downgrade();
        let exit = scope.downgrade();
        let bar = scope.downgrade();
        let search = scope.downgrade();
        eprintln!(
            "Glory menu probe: menubar / managed menu / context region / explicit palette shortcut"
        );
        view()
            .class("menu-probe")
            .on(EventKind::Palette, move |_| {
                open.revise(|v| *v = false);
                palette.revise(|v| *v = !*v);
            })
            .fill((
                text().content("Native menus and context requests"),
                view()
                    .class("menu-bar")
                    .role(Role::Menubar)
                    .name("Application actions")
                    .semantics(Some(Box::new(Semantics {
                        id: Some(name("menu-bar")),
                        navigation: Some(Orientation::Horizontal),
                        ..Default::default()
                    })))
                    .on(EventKind::Navigate, move |event| {
                        if let Event::Navigate { command, .. } = event {
                            let index = match command {
                                Navigation::First => 0,
                                Navigation::Last => 1,
                                _ => 1 - active.get(),
                            };
                            active.revise(|v| *v = index);
                            if let Some(scope) = bar.upgrade()
                                && let Some(node) = roots[index].get()
                            {
                                let _ = scope.action(Action::Focus(node));
                            }
                        }
                    })
                    .fill(triggers),
                button()
                    .label("Open command input / Ctrl+K")
                    .on(EventKind::Activate, move |_| {
                        open.revise(|v| *v = false);
                        palette.revise(|v| *v = true);
                    }),
                view()
                    .role(Role::Menu)
                    .name("Actions")
                    .class("menu-panel")
                    .focus(FocusMode::Programmatic)
                    .node_ref(panel)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(name("menu-panel")),
                            hidden: !open.get(),
                            navigation: Some(Orientation::Vertical),
                            typeahead: true,
                            layer: Some(Layer {
                                modal: false,
                                focus: LayerFocus::Manage,
                                placement: point.get(),
                                anchor: Some(name(if context.get() {
                                    "menu-context"
                                } else if active.get() == 0 {
                                    "menu-file"
                                } else {
                                    "menu-edit"
                                })),
                                return_focus: Some(name(if context.get() {
                                    "menu-context"
                                } else if active.get() == 0 {
                                    "menu-file"
                                } else {
                                    "menu-edit"
                                })),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                    .on(EventKind::Menu, move |event| {
                        if let Event::Menu {
                            command: MenuCommand::Exit { backwards },
                            ..
                        } = event
                        {
                            open.revise(|v| *v = false);
                            if let Some(scope) = exit.upgrade()
                                && let Some(node) = panel.get()
                            {
                                let _ = scope.action(Action::ExitMenu { node, backwards });
                            }
                        }
                    })
                    .on(EventKind::Navigate, move |event| {
                        if let Event::Navigate { command, .. } = event {
                            let index = match command {
                                Navigation::First | Navigation::PagePrevious => 0,
                                Navigation::Last | Navigation::PageNext => 3,
                                Navigation::Previous => match current.get() {
                                    0 => 3,
                                    2 => 0,
                                    _ => 2,
                                },
                                _ => match current.get() {
                                    0 => 2,
                                    2 => 3,
                                    _ => 0,
                                },
                            };
                            current.revise(|v| *v = index);
                            if let Some(scope) = weak.upgrade()
                                && let Some(node) = if index == 3 {
                                    check_ref.get()
                                } else {
                                    options[index].get()
                                }
                            {
                                let _ = scope.action(Action::Focus(node));
                            }
                        }
                    })
                    .on(EventKind::Typeahead, move |event| {
                        if let Event::Typeahead { text, .. } = event
                            && let Some(index) =
                                [LABELS[0], LABELS[1], LABELS[2], "Checkable menu action"]
                                    .iter()
                                    .enumerate()
                                    .find_map(|(i, label)| {
                                        (i != 1
                                            && label
                                                .to_lowercase()
                                                .starts_with(&text.to_lowercase()))
                                        .then_some(i)
                                    })
                        {
                            current.revise(|v| *v = index);
                            if let Some(scope) = search.upgrade()
                                && let Some(node) = if index == 3 {
                                    check_ref.get()
                                } else {
                                    options[index].get()
                                }
                            {
                                let _ = scope.action(Action::Focus(node));
                            }
                        }
                    })
                    .fill((
                        group().fill(items),
                        check()
                            .node_ref(check_ref)
                            .focus(current.map(|current| {
                                if *current == 3 {
                                    FocusMode::TabStop
                                } else {
                                    FocusMode::Programmatic
                                }
                            }))
                            .label("Checkable menu action")
                            .checked(checked)
                            .semantics(Some(Box::new(Semantics {
                                navigation_item: true,
                                menu_item: Some(MenuItemSemantics {
                                    kind: MenuItemKind::Checkbox,
                                    owner: name("menu-panel"),
                                }),
                                ..Default::default()
                            })))
                            .on(EventKind::FocusChanged, move |event| {
                                if event == Event::FocusChanged(true) {
                                    current.revise(|v| *v = 3);
                                }
                            })
                            .on(EventKind::CheckedChanged, move |event| {
                                if let Event::CheckedChanged(next) = event {
                                    checked.revise(|v| *v = next);
                                }
                            }),
                    )),
                view()
                    .role(Role::Group)
                    .name("Context region")
                    .class("menu-context")
                    .focus(FocusMode::TabStop)
                    .semantics(Some(Box::new(Semantics {
                        id: Some(name("menu-context")),
                        context_menu: Some(ContextEditPolicy::Native),
                        ..Default::default()
                    })))
                    .on(EventKind::ContextMenu, move |event| {
                        if let Event::ContextMenu {
                            point: requested, ..
                        } = event
                        {
                            context.revise(|v| *v = true);
                            point.revise(|v| *v = LayerPlacement::ContextPoint(requested));
                            open.revise(|v| *v = true);
                        }
                    })
                    .fill(text().content("Right click / Shift+F10\nTouch long-press stays native")),
                view()
                    .role(Role::Dialog)
                    .name("Command input")
                    .focus(FocusMode::Programmatic)
                    .class("menu-palette")
                    .semantics(palette.map(|open| {
                        Some(Box::new(Semantics {
                            id: Some(name("menu-palette")),
                            hidden: !*open,
                            layer: Some(Layer {
                                initial_focus: Some(name("menu-search")),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .on(EventKind::Dismiss, move |_| palette.revise(|v| *v = false))
                    .fill((
                        text().content("Command input"),
                        input()
                            .name("Search commands")
                            .value(draft)
                            .semantics(Some(Box::new(Semantics {
                                id: Some(name("menu-search")),
                                placeholder: Some("Search / 中文".into()),
                                ..Default::default()
                            })))
                            .on(EventKind::ValueChanged, move |event| {
                                if let Event::ValueChanged { value, .. } = event {
                                    draft.revise(|v| *v = value);
                                }
                            }),
                        button()
                            .label("Close")
                            .on(EventKind::Activate, move |_| palette.revise(|v| *v = false)),
                    )),
            ))
            .show_in(scope)
    })
}
