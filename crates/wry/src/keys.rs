//! Derive synchronous WebView default-prevention tables from the render contract.
//! IPC carries a finite binding and a generational physical origin. The normal
//! Memory validation still checks ownership, enablement and current ancestry.
use glory_render::*;
use serde_json::{Map, Value};
use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq)]
enum Command {
    Grid(GridCommand),
    Tree(TreeCommand),
    Scroll(ScrollCommand),
    Spin(RangeCommand),
    Range(RangeCommand),
    Navigate(Navigation),
    Choice(ChoiceCommand),
    Menu(MenuCommand),
    Panel(NavigationPanelCommand),
}

impl Command {
    fn event(&self, from: NodeId) -> Event {
        match self {
            Self::Range(command) => Event::Range {
                from,
                command: *command,
            },
            Self::Navigate(command) => Event::Navigate {
                from,
                command: *command,
            },
            Self::Grid(command) => Event::GridNavigate {
                from,
                command: *command,
            },
            Self::Tree(command) => Event::TreeNavigate {
                from,
                command: command.clone(),
            },
            Self::Spin(command) => Event::Spin {
                from,
                command: *command,
            },
            Self::Scroll(command) => Event::ScrollKeyboard {
                from,
                command: *command,
            },
            Self::Choice(command) => Event::Choice {
                from,
                command: *command,
            },
            Self::Menu(command) => Event::Menu {
                from,
                command: *command,
            },
            Self::Panel(command) => Event::NavigationPanel {
                from,
                command: *command,
            },
        }
    }
}

struct Bindings {
    profiles: Map<String, Value>,
    commands: Vec<Command>,
}
impl Bindings {
    fn insert(&mut self, name: String, decode: impl Fn(KeyboardInput<'_>) -> Option<Command>) {
        let mut bindings = Map::new();
        for key in [
            "ArrowDown",
            "ArrowUp",
            "ArrowLeft",
            "ArrowRight",
            "PageDown",
            "PageUp",
            "Home",
            "End",
            "Escape",
            "Enter",
            " ",
            "Tab",
            "Backspace",
        ] {
            for flags in 0..8 {
                let input = KeyboardInput {
                    key,
                    shift: flags & 1 != 0,
                    repeat: flags & 2 != 0,
                    alt: flags & 4 != 0,
                    ..Default::default()
                };
                if let Some(command) = decode(input) {
                    let id = self
                        .commands
                        .iter()
                        .position(|item| item == &command)
                        .unwrap_or_else(|| {
                            self.commands.push(command);
                            self.commands.len() - 1
                        });
                    bindings.insert(format!("{flags}{key}"), id.into());
                }
            }
        }
        self.profiles.insert(name, bindings.into());
    }
    fn new() -> Self {
        let mut table = Self {
            profiles: Map::new(),
            commands: Vec::new(),
        };
        for (dir, direction) in [("ltr", Direction::Ltr), ("rtl", Direction::Rtl)] {
            table.insert(format!("grid-{dir}"), |input| {
                GridCommand::key(direction, input).map(Command::Grid)
            });
            table.insert(format!("tree-{dir}"), |input| {
                TreeCommand::key(
                    TreeNavigation {
                        direction,
                        typeahead: false,
                        ..Default::default()
                    },
                    input,
                    0,
                )
                .map(Command::Tree)
            });
            for axis in [Orientation::Horizontal, Orientation::Vertical] {
                table.insert(format!("range-{}-{dir}", axis.as_str()), |input| {
                    RangeCommand::slider_key(axis, direction, input).map(Command::Range)
                });
                table.insert(format!("navigate-{}-{dir}", axis.as_str()), |input| {
                    axis.key_with_direction(direction, input)
                        .map(Command::Navigate)
                });
            }
            for trigger in [false, true] {
                table.insert(format!("menu-{dir}-{trigger}"), |input| {
                    input.menu(direction, trigger).map(Command::Menu)
                });
                for inside in [false, true] {
                    table.insert(format!("panel-{dir}-{trigger}-{inside}"), |input| {
                        input
                            .navigation_panel(direction, trigger, inside)
                            .map(Command::Panel)
                    });
                }
            }
        }
        for axis in [Orientation::Horizontal, Orientation::Vertical] {
            table.insert(format!("splitter-{}", axis.as_str()), |input| {
                RangeCommand::splitter_key(axis, input).map(Command::Range)
            });
        }
        for axes in [
            ScrollAxes::Horizontal,
            ScrollAxes::Vertical,
            ScrollAxes::Both,
        ] {
            table.insert(format!("scroll-{}", axes.as_str()), |input| {
                axes.key(input).map(Command::Scroll)
            });
        }
        for policy in [
            TagSeparators::Enter,
            TagSeparators::EnterComma,
            TagSeparators::EnterCommaSemicolon,
        ] {
            for empty in [false, true] {
                let mut shortcuts = Map::new();
                for key in ["Backspace", "Enter", ",", ";"] {
                    if let Some(command) = policy.key(
                        KeyboardInput {
                            key,
                            editable: true,
                            ..Default::default()
                        },
                        empty,
                    ) {
                        let value = match command {
                            TagKey::Commit => "commit",
                            TagKey::RemoveLast => "remove_last",
                        };
                        shortcuts.insert(key.into(), value.into());
                    }
                }
                table.profiles.insert(
                    format!("tags-{}-{empty}", policy.as_str()),
                    shortcuts.into(),
                );
            }
        }
        for min in [false, true] {
            for max in [false, true] {
                table.insert(format!("spin-{min}-{max}"), |input| {
                    RangeCommand::spin_key(min, max, input).map(Command::Spin)
                });
            }
        }
        table.insert("menu-search".into(), |input| {
            input.menu_search().map(Command::Menu)
        });
        for (name, kind) in [
            ("select", ComboboxKind::Select),
            ("editable", ComboboxKind::Editable),
            ("autocomplete", ComboboxKind::Autocomplete),
            ("multiple", ComboboxKind::Multiple),
        ] {
            for expanded in [false, true] {
                for empty in [false, true] {
                    table.insert(format!("choice-{name}-{expanded}-{empty}"), |input| {
                        input
                            .choice_remove_last(kind, empty)
                            .or_else(|| input.choice(kind, expanded))
                            .map(Command::Choice)
                    });
                }
            }
        }
        table
    }
}

static BINDINGS: LazyLock<Bindings> = LazyLock::new(Bindings::new);
pub(super) fn profiles() -> &'static Map<String, Value> {
    &BINDINGS.profiles
}
pub(super) fn event(binding: u16, from: NodeId, nested: bool) -> Result<Event, Error> {
    let command = BINDINGS
        .commands
        .get(usize::from(binding))
        .ok_or(Error::InvalidEvent("unknown Wry key binding"))?;
    if nested != matches!(command, Command::Tree(_) | Command::Grid(_)) {
        return Err(Error::InvalidEvent("Wry key binding origin"));
    }
    Ok(command.event(from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_collection_bindings_accept_a_distinct_origin() {
        let origin = NodeIds::new(RuntimeId::new()).allocate();
        let binding =
            |profile: &str, shortcut: &str| profiles()[profile][shortcut].as_u64().unwrap() as u16;
        let grid = binding("grid-rtl", "0ArrowLeft");
        assert!(event(grid, origin, false).is_err());
        assert_eq!(
            event(grid, origin, true).unwrap(),
            Event::GridNavigate {
                from: origin,
                command: GridCommand::Right
            }
        );
        let spin = binding("spin-false-true", "0End");
        assert!(event(spin, origin, true).is_err());
        assert_eq!(
            event(spin, origin, false).unwrap(),
            Event::Spin {
                from: origin,
                command: RangeCommand::Maximum
            }
        );
        assert!(profiles()["spin-false-true"].get("0Home").is_none());
        assert!(event(u16::MAX, origin, false).is_err());
    }
}
