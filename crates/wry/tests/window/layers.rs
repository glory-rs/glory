use glory_render::{
    AnchorAlign, AnchorArrow, AnchorPlacement, AnchorSide, CollapseConfig, Event, EventKind,
    FocusMode, Layer, LayerPlacement, Role, SemanticId, Semantics,
};
use glory_ui::{button, group, text, view};
use glory_wry::shell::WindowApp;

fn id(value: &str) -> Option<SemanticId> {
    Some(SemanticId::new(value).unwrap())
}

fn mount_sheet(app: &WindowApp) -> glory_ui::Result {
    let scope = app.app().scope();
    let open = scope.cage(false);
    let state = scope.cage((0usize, 0u64));
    app.mount(
        group().fill((
            button()
                .class("sheet-trigger")
                .label("Open snap sheet")
                .on(EventKind::Activate, move |_| open.revise(|v| *v = true)),
            view()
                .class("snap-sheet")
                .role(Role::Dialog)
                .focus(FocusMode::Programmatic)
                .semantics(scope.bond(move || {
                    let (current, revision) = state.get();
                    Some(Box::new(Semantics {
                        hidden: !open.get(),
                        id: id("wry-sheet"),
                        layer: Some(Layer {
                            placement: LayerPlacement::Bottom,
                            sheet: Some(glory_render::SheetConfig {
                                fractions: vec![0.4, 0.8],
                                current,
                                max_extent: 500.,
                                settle_millis: 60,
                                dismissible: true,
                                revision,
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Sheet, move |event| {
                    if let Event::Sheet { command, .. } = event {
                        match command {
                            glory_render::SheetCommand::Dismiss => open.revise(|v| *v = false),
                            glory_render::SheetCommand::Snap(index) => {
                                state.revise(|v| *v = (index, v.1 + 1))
                            }
                        }
                    }
                })
                .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                .fill((
                    view()
                        .class("snap-grip")
                        .semantics(Some(Box::new(Semantics {
                            sheet_grip: true,
                            ..Default::default()
                        })))
                        .style(
                            glory_render::style::Style::new()
                                .height(glory_render::style::Length::Px(24.)),
                        ),
                    button()
                        .class("snap-next")
                        .label("Expand sheet")
                        .on(EventKind::Activate, move |_| {
                            state.revise(|v| *v = (1, v.1 + 1))
                        }),
                )),
        )),
    )?;
    Ok(())
}

fn mount_windows(app: &WindowApp) -> glory_ui::Result {
    let scope = app.app().scope();
    let visible = scope.cage(false);
    let reference = scope.cage(None);
    let weak = scope.downgrade();
    let panel = |name: &'static str| {
        view()
            .class(name)
            .role(Role::Dialog)
            .name(name)
            .focus(FocusMode::Programmatic)
            .semantics(visible.map(move |open| {
                Some(Box::new(Semantics {
                    id: id(name),
                    hidden: !*open,
                    layer: Some(Layer {
                        modal: false,
                        placement: LayerPlacement::Viewport(glory_render::ViewportPlacement {
                            x: 20.,
                            y: 24.,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }))
            }))
            .style(
                glory_render::style::Style::new()
                    .width(glory_render::style::Length::Px(150.))
                    .height(glory_render::style::Length::Px(80.)),
            )
    };
    app.mount(
        group().fill((
            button()
                .class("window-toggle")
                .label("Toggle floating windows")
                .on(EventKind::Activate, move |_| {
                    visible.revise(|value| *value = !*value)
                }),
            button().class("window-raise").label("Raise first").on(
                EventKind::Activate,
                move |_| {
                    weak.upgrade()
                        .unwrap()
                        .action(glory_render::Action::RaiseLayer(reference.get().unwrap()))
                        .unwrap()
                },
            ),
            panel("window-first")
                .node_ref(reference)
                .fill(button().label("First editor")),
            panel("window-last").fill(button().label("Second editor")),
        )),
    )?;
    Ok(())
}

pub(super) fn mount(app: &WindowApp) -> glory_ui::Result {
    let scope = app.app().scope();
    let open = scope.cage(false);
    let revision = scope.cage(0u64);
    let completed = scope.cage(String::new());
    mount_sheet(app)?;
    mount_windows(app)?;
    let presence = scope.cage(String::new());
    let trapped = scope.cage(false);
    app.mount(
        group().fill((
            button()
                .class("popup-trigger")
                .label("Open anchored popup")
                .semantics(Some(Box::new(Semantics {
                    id: id("wry-trigger"),
                    ..Default::default()
                })))
                .on(EventKind::Activate, move |_| {
                    open.revise(|value| *value = true)
                }),
            view()
                .class("popup-panel")
                .role(Role::Dialog)
                .focus(FocusMode::Programmatic)
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: id("wry-popup"),
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: false,
                            placement: LayerPlacement::Anchor(AnchorPlacement {
                                side: AnchorSide::Bottom,
                                align: AnchorAlign::Start,
                                match_width: true,
                                arrow: Some(AnchorArrow::default()),
                                ..Default::default()
                            }),
                            anchor: id("wry-trigger"),
                            initial_focus: id("wry-confirm"),
                            return_focus: id("wry-trigger"),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::Dismiss, move |_| {
                    open.revise(|value| *value = false)
                })
                .fill(
                    button()
                        .class("popup-confirm")
                        .label("Confirm")
                        .semantics(Some(Box::new(Semantics {
                            id: id("wry-confirm"),
                            ..Default::default()
                        }))),
                ),
            button()
                .class("collapse-toggle")
                .label("Toggle height")
                .on(EventKind::Activate, move |_| {
                    revision.revise(|value| *value += 1)
                }),
            view()
                .class("collapse-observed")
                .semantics(revision.map(|revision| {
                    Some(Box::new(Semantics {
                        hidden: revision % 2 != 0,
                        inert: revision % 2 != 0,
                        collapse: Some(CollapseConfig {
                            open: revision % 2 == 0,
                            duration_millis: 60,
                            revision: *revision,
                        }),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::CollapseComplete, move |event| {
                    if let Event::CollapseComplete { revision } = event {
                        completed.revise(|value| *value = revision.to_string())
                    }
                })
                .fill(text().content("Collapse content")),
            text().class("collapse-receipt").content(completed),
            view()
                .class("presence-observed")
                .semantics(revision.map(|revision| {
                    Some(Box::new(Semantics {
                        hidden: revision % 2 != 0,
                        inert: revision % 2 != 0,
                        presence: Some(Box::new(glory_render::PresenceConfig {
                            open: revision % 2 == 0,
                            duration_millis: 60,
                            revision: *revision,
                            ..Default::default()
                        })),
                        ..Default::default()
                    }))
                }))
                .on(EventKind::PresenceComplete, move |event| {
                    if let Event::PresenceComplete { revision } = event {
                        presence.revise(|value| *value = revision.to_string())
                    }
                })
                .fill(text().content("Presence content")),
            text().class("presence-receipt").content(presence),
            button()
                .class("scope-trigger")
                .label("Activate focus scope")
                .on(EventKind::Activate, move |_| {
                    trapped.revise(|value| *value = true)
                }),
            view()
                .class("focus-scope")
                .focus(FocusMode::Programmatic)
                .semantics(trapped.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(glory_render::FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill((
                    button().class("scope-first").label("First"),
                    button()
                        .class("scope-last")
                        .label("Done")
                        .on(EventKind::Activate, move |_| {
                            trapped.revise(|value| *value = false)
                        }),
                )),
            view()
                .class("looping")
                .semantics(Some(Box::new(Semantics {
                    inert: true,
                    translation: Some(glory_render::TranslationMotion {
                        kind: glory_render::TranslationKind::Loop {
                            from: glory_render::TranslationOffset::ZERO,
                            to: glory_render::TranslationOffset::new(40., 0.),
                            period_millis: 400,
                        },
                        paused: false,
                    }),
                    ..Default::default()
                })))
                .fill(text().content("Moving")),
        )),
    )?;
    Ok(())
}
