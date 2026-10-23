//! Finite application commands; hosts never expose arbitrary typed characters.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardCommand {
    Submit,
    Palette,
    Search,
    Help,
    Dismiss,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct KeyboardInput<'a> {
    pub key: &'a str,
    pub control: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
    pub repeat: bool,
    pub composing: bool,
    pub editable: bool,
}

impl KeyboardInput<'_> {
    pub fn command(self) -> Option<KeyboardCommand> {
        if self.repeat || self.composing || self.alt {
            return None;
        }
        if self.key == "Escape" {
            return Some(KeyboardCommand::Dismiss);
        }
        if self.editable {
            return (self.key == "Enter" && !self.shift).then_some(KeyboardCommand::Submit);
        }
        let chord = self.control || self.meta;
        if chord && !self.shift && self.key.eq_ignore_ascii_case("k") {
            Some(KeyboardCommand::Palette)
        } else if chord && !self.shift && self.key.eq_ignore_ascii_case("f") {
            Some(KeyboardCommand::Search)
        } else if (chord && self.key == "/") || self.key == "?" || (self.shift && self.key == "/") {
            Some(KeyboardCommand::Help)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composition_repeat_editing_and_modifiers_preserve_native_text() {
        let input = KeyboardInput {
            key: "Enter",
            editable: true,
            ..Default::default()
        };
        assert_eq!(input.command(), Some(KeyboardCommand::Submit));
        assert_eq!(
            KeyboardInput {
                control: true,
                ..input
            }
            .command(),
            Some(KeyboardCommand::Submit)
        );
        for ignored in [
            KeyboardInput {
                shift: true,
                ..input
            },
            KeyboardInput { alt: true, ..input },
            KeyboardInput {
                repeat: true,
                ..input
            },
            KeyboardInput {
                composing: true,
                ..input
            },
        ] {
            assert_eq!(ignored.command(), None);
        }
        assert_eq!(
            KeyboardInput {
                key: "?",
                editable: true,
                ..Default::default()
            }
            .command(),
            None
        );
        assert_eq!(
            KeyboardInput {
                key: "k",
                control: true,
                editable: true,
                ..Default::default()
            }
            .command(),
            None
        );
        assert_eq!(
            KeyboardInput {
                key: "k",
                control: true,
                ..Default::default()
            }
            .command(),
            Some(KeyboardCommand::Palette)
        );
        assert_eq!(
            KeyboardInput {
                key: "f",
                meta: true,
                ..Default::default()
            }
            .command(),
            Some(KeyboardCommand::Search)
        );
    }
}
